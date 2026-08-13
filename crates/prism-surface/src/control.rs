//! Inbound: what the operator did, as a value.
//!
//! One [`MidiMessage`] in, one [`ControlEvent`] out — or nothing, counted. This
//! is the top half of layer 1 in `docs/MCU_MAPPING.md` §1: it knows the note
//! numbers (through [`McuProfile`], never as a literal) and it knows nothing
//! whatever about PrismDMX. `GlobalButton::Play` is a note number here; that it
//! starts an executor is layer 3's opinion.
//!
//! # Two encodings worth reading twice
//!
//! **A release is a Note On with velocity 0**, not a Note Off. That is what the
//! surface sends and what [`ControlEvent::to_midi`] writes back, so the round
//! trip is byte-equal. A real Note Off is accepted on the way in — a device is
//! entitled to send one — and normalises to the same event, which means it is
//! an *alias* rather than a second canonical form. The round-trip table says
//! which rows are which.
//!
//! **The V-Pots and the jog wheel are relative, in sign-magnitude**: bit 6 is
//! the sign and bits 0…5 the number of detents, so `0x01` is one step clockwise
//! and `0x41` one step back. Two's complement would decode `0x41` as +65 and
//! send a fader to the top, which is the kind of fault that looks like a broken
//! encoder rather than a broken codec.
//!
//! Whether a fast turn sends one step per message or a larger magnitude is
//! **unconfirmed** (`docs/MCU_MAPPING.md` §2.1 and §7). The codec passes the
//! magnitude through unchanged and interprets nothing: if the device turns out
//! to accelerate, that is a curve in layer 2 and no change here.

use crate::midi::{EncodeError, MidiMessage};
use crate::profile::{Fader, GlobalButton, McuProfile, StripButton};
use core::fmt;

/// Velocity the surface sends for a press, and the one this crate writes.
pub const PRESS_VELOCITY: u8 = 0x7F;

/// Velocity for a release.
pub const RELEASE_VELOCITY: u8 = 0x00;

/// The sign bit of a relative control's value.
const SIGN_BIT: u8 = 0x40;

/// The largest number of detents one relative message can carry.
pub const MAX_RELATIVE_STEPS: i8 = 0x3F;

/// Which button, named by where it is rather than by what it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ButtonId {
    /// A button on a channel strip, counted from the left.
    Strip {
        /// Strip index, 0 at the left.
        strip: u8,
        /// Which of the strip's buttons.
        button: StripButton,
    },
    /// A button that belongs to the panel rather than to a strip.
    Global(GlobalButton),
}

impl fmt::Display for ButtonId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Strip { strip, button } => write!(f, "Strip[{strip}].Button.{button}"),
            Self::Global(button) => write!(f, "Global.{button:?}"),
        }
    }
}

/// Something the operator did to the surface.
///
/// `Copy` and free of owned fields on purpose: an inbound event crosses a
/// thread boundary on its way to the command bus, and a type that allocates on
/// the way would put an allocation on the path `ARCHITECTURE_SPEC.md` §4.3
/// budgets in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlEvent {
    /// A button went down or came up.
    Button {
        /// Which button.
        button: ButtonId,
        /// `true` for a press.
        pressed: bool,
    },
    /// A fader's touch sensor. The reason [`Feedback`](crate::Feedback) must
    /// not drive that fader until it is released — `docs/MCU_MAPPING.md` §5.1.
    Touch {
        /// Which fader.
        fader: Fader,
        /// `true` while a hand is on it.
        touched: bool,
    },
    /// A fader moved. 14-bit, 0…16383.
    Move {
        /// Which fader.
        fader: Fader,
        /// Position, 0…16383. Linear here; the **printed scale is a dB scale**
        /// whose 0 mark sits at roughly 77 % of travel, which is decoration as
        /// far as a lighting desk is concerned (`docs/MCU_MAPPING.md` §2.2).
        position: u16,
    },
    /// A V-Pot was turned, by a signed number of detents.
    VPot {
        /// Strip index, 0 at the left.
        strip: u8,
        /// Detents, positive clockwise. `-63`…`63`.
        steps: i8,
    },
    /// The jog wheel was turned, in the same encoding.
    Jog {
        /// Detents, positive clockwise. `-63`…`63`.
        steps: i8,
    },
}

/// The largest position a 14-bit fader can report.
pub const FADER_MAX: u16 = 0x3FFF;

impl ControlEvent {
    /// Reads an event out of a MIDI message, or `None` if this surface has no
    /// such control.
    ///
    /// `None` is not an error — it is a message the profile does not describe,
    /// and [`McuCodec`](crate::McuCodec) counts it so that a surface sending
    /// something unexpected is visible rather than merely ineffective.
    #[must_use]
    pub fn from_midi(profile: &McuProfile, message: &MidiMessage<'_>) -> Option<Self> {
        match *message {
            MidiMessage::NoteOn {
                channel,
                note,
                velocity,
            } if channel == profile.channel => {
                Self::from_note(profile, note, velocity != RELEASE_VELOCITY)
            }
            // A device is entitled to send a real Note Off. It means the same
            // thing and normalises to the same event.
            MidiMessage::NoteOff { channel, note, .. } if channel == profile.channel => {
                Self::from_note(profile, note, false)
            }
            MidiMessage::ControlChange {
                channel,
                controller,
                value,
            } if channel == profile.channel => {
                Self::from_control_change(profile, controller, value)
            }
            MidiMessage::PitchBend { channel, value } => Some(Self::Move {
                fader: profile.fader_on_channel(channel)?,
                position: value,
            }),
            _ => None,
        }
    }

    /// A note number and its press state.
    fn from_note(profile: &McuProfile, note: u8, pressed: bool) -> Option<Self> {
        if let Some(fader) = profile.touch_at(note) {
            return Some(Self::Touch {
                fader,
                touched: pressed,
            });
        }
        if let Some((strip, button)) = profile.strip_button_at(note) {
            return Some(Self::Button {
                button: ButtonId::Strip { strip, button },
                pressed,
            });
        }
        profile.button_at(note).map(|button| Self::Button {
            button: ButtonId::Global(button),
            pressed,
        })
    }

    /// A controller number and its value.
    fn from_control_change(profile: &McuProfile, controller: u8, value: u8) -> Option<Self> {
        if controller == profile.jog_cc {
            return Some(Self::Jog {
                steps: relative_steps(value),
            });
        }
        profile.vpot_strip_at(controller).map(|strip| Self::VPot {
            strip,
            steps: relative_steps(value),
        })
    }

    /// The MIDI message this event is written as.
    ///
    /// The canonical form: a release is a Note On with velocity 0, and a press
    /// is velocity 127. Feeding the result back through
    /// [`from_midi`](Self::from_midi) returns this event unchanged.
    ///
    /// # Errors
    ///
    /// [`EncodeError::NoSuchControl`] for a control this surface does not have
    /// — a ninth strip, a button the profile has no note for —
    /// and [`EncodeError::OutOfRange`] for a position or a step count that does
    /// not fit the wire.
    pub fn to_midi(&self, profile: &McuProfile) -> Result<MidiMessage<'static>, EncodeError> {
        match *self {
            Self::Button { button, pressed } => {
                let note = note_of(profile, button).ok_or(EncodeError::NoSuchControl)?;
                Ok(note_message(profile, note, pressed))
            }
            Self::Touch { fader, touched } => {
                let note = profile
                    .touch_note_of(fader)
                    .ok_or(EncodeError::NoSuchControl)?;
                Ok(note_message(profile, note, touched))
            }
            Self::Move { fader, position } => {
                let channel = profile
                    .fader_channel(fader)
                    .ok_or(EncodeError::NoSuchControl)?;
                if position > FADER_MAX {
                    return Err(EncodeError::OutOfRange {
                        field: "fader position",
                        value: u32::from(position),
                    });
                }
                Ok(MidiMessage::PitchBend {
                    channel,
                    value: position,
                })
            }
            Self::VPot { strip, steps } => {
                let controller = profile
                    .vpot_controller(strip)
                    .ok_or(EncodeError::NoSuchControl)?;
                Ok(MidiMessage::ControlChange {
                    channel: profile.channel,
                    controller,
                    value: relative_value(steps)?,
                })
            }
            Self::Jog { steps } => Ok(MidiMessage::ControlChange {
                channel: profile.channel,
                controller: profile.jog_cc,
                value: relative_value(steps)?,
            }),
        }
    }

    /// Writes this event into a caller-provided buffer and returns how many
    /// bytes it used.
    ///
    /// # Errors
    ///
    /// As [`to_midi`](Self::to_midi), plus
    /// [`EncodeError::BufferTooSmall`].
    pub fn encode_into(&self, profile: &McuProfile, buf: &mut [u8]) -> Result<usize, EncodeError> {
        self.to_midi(profile)?.encode_into(buf)
    }
}

/// The note a button is on, whichever table it lives in.
fn note_of(profile: &McuProfile, button: ButtonId) -> Option<u8> {
    match button {
        ButtonId::Strip { strip, button } => profile.strip_note(strip, button),
        ButtonId::Global(button) => profile.note_of(button),
    }
}

/// A press or a release, as the surface writes it.
const fn note_message(profile: &McuProfile, note: u8, pressed: bool) -> MidiMessage<'static> {
    MidiMessage::NoteOn {
        channel: profile.channel,
        note,
        velocity: if pressed {
            PRESS_VELOCITY
        } else {
            RELEASE_VELOCITY
        },
    }
}

/// Decodes a relative control's sign-magnitude byte.
///
/// Total: every one of the 128 data-byte values is a valid step count, which is
/// why this returns an `i8` and not an `Option`.
#[must_use]
pub const fn relative_steps(value: u8) -> i8 {
    let magnitude = (value & 0x3F) as i8;
    if value & SIGN_BIT == 0 {
        magnitude
    } else {
        -magnitude
    }
}

/// Encodes a step count back into a sign-magnitude byte.
///
/// # Errors
///
/// [`EncodeError::OutOfRange`] beyond ±[`MAX_RELATIVE_STEPS`]: one message
/// cannot carry more, and silently clamping would turn a fast turn into a slow
/// one without anybody being told.
pub const fn relative_value(steps: i8) -> Result<u8, EncodeError> {
    let magnitude = steps.unsigned_abs();
    if magnitude > MAX_RELATIVE_STEPS as u8 {
        return Err(EncodeError::OutOfRange {
            field: "relative steps",
            value: magnitude as u32,
        });
    }
    if steps < 0 {
        Ok(SIGN_BIT | magnitude)
    } else {
        Ok(magnitude)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ButtonId, ControlEvent, FADER_MAX, MAX_RELATIVE_STEPS, relative_steps, relative_value,
    };
    use crate::midi::{EncodeError, MidiMessage};
    use crate::profile::{Fader, GlobalButton, StripButton, X_TOUCH};

    #[test]
    fn a_release_is_a_note_on_with_velocity_zero() {
        // What the surface actually sends. A codec that wrote a real Note Off
        // would still round-trip through itself and would light no LED on the
        // desk, because the device is listening for 0x90.
        let event = ControlEvent::Button {
            button: ButtonId::Strip {
                strip: 5,
                button: StripButton::Select,
            },
            pressed: false,
        };
        assert_eq!(
            event.to_midi(&X_TOUCH),
            Ok(MidiMessage::NoteOn {
                channel: 0,
                note: 29,
                velocity: 0
            })
        );
    }

    #[test]
    fn a_real_note_off_is_accepted_and_means_the_same_thing() {
        // Decode-only: it normalises onto the canonical form above, so it is an
        // alias rather than a second way of writing a release.
        let alias = MidiMessage::NoteOff {
            channel: 0,
            note: 29,
            velocity: 0x40,
        };
        let canonical = MidiMessage::NoteOn {
            channel: 0,
            note: 29,
            velocity: 0,
        };
        assert_eq!(
            ControlEvent::from_midi(&X_TOUCH, &alias),
            ControlEvent::from_midi(&X_TOUCH, &canonical)
        );
    }

    #[test]
    fn any_non_zero_velocity_is_a_press() {
        // The surface sends 127, but the rule is "not zero" - a device with a
        // velocity-sensitive button would otherwise report a soft press as a
        // release, which is a button that works only when hit hard.
        for velocity in [1, 0x40, 0x7F] {
            let message = MidiMessage::NoteOn {
                channel: 0,
                note: 94,
                velocity,
            };
            assert_eq!(
                ControlEvent::from_midi(&X_TOUCH, &message),
                Some(ControlEvent::Button {
                    button: ButtonId::Global(GlobalButton::Play),
                    pressed: true
                })
            );
        }
    }

    #[test]
    fn the_relative_encoding_is_sign_magnitude_and_not_twos_complement() {
        // 0x41 is minus one. Read as two's complement it is plus 65, and a
        // V-Pot nudged backwards would throw its parameter to the top.
        assert_eq!(relative_steps(0x01), 1);
        assert_eq!(relative_steps(0x41), -1);
        assert_eq!(relative_steps(0x3F), 63);
        assert_eq!(relative_steps(0x7F), -63);
        assert_eq!(relative_value(1), Ok(0x01));
        assert_eq!(relative_value(-1), Ok(0x41));
        assert_eq!(relative_value(63), Ok(0x3F));
        assert_eq!(relative_value(-63), Ok(0x7F));
    }

    #[test]
    fn every_relative_byte_decodes_and_all_but_the_signed_zero_write_back() {
        // 0x40 is minus zero: it decodes to no movement and is written as 0x00,
        // which is the one place the relative encoding is not a bijection.
        for value in 0..=0x7Fu8 {
            let steps = relative_steps(value);
            let written = relative_value(steps).expect("a decoded step count fits");
            let expected = if value == 0x40 { 0x00 } else { value };
            assert_eq!(written, expected, "0x{value:02X} decoded to {steps}");
        }
    }

    #[test]
    fn a_turn_too_large_for_one_message_is_refused_rather_than_clamped() {
        // Clamping would turn a fast turn into a slow one with nobody told.
        assert_eq!(
            relative_value(MAX_RELATIVE_STEPS + 1),
            Err(EncodeError::OutOfRange {
                field: "relative steps",
                value: 64
            })
        );
        assert_eq!(
            relative_value(-128),
            Err(EncodeError::OutOfRange {
                field: "relative steps",
                value: 128
            })
        );
    }

    #[test]
    fn a_v_pot_turn_carries_its_strip() {
        let message = MidiMessage::ControlChange {
            channel: 0,
            controller: 19,
            value: 0x42,
        };
        assert_eq!(
            ControlEvent::from_midi(&X_TOUCH, &message),
            Some(ControlEvent::VPot {
                strip: 3,
                steps: -2
            })
        );
        assert_eq!(
            ControlEvent::VPot {
                strip: 3,
                steps: -2
            }
            .to_midi(&X_TOUCH),
            Ok(message)
        );
    }

    #[test]
    fn the_jog_wheel_is_not_a_ninth_v_pot() {
        // CC 60 sits above the V-Pot block but below the 7-segment one, and a
        // decoder that computed `controller - vpot_cc` without checking the
        // range would report it as strip 44.
        let message = MidiMessage::ControlChange {
            channel: 0,
            controller: 60,
            value: 0x03,
        };
        assert_eq!(
            ControlEvent::from_midi(&X_TOUCH, &message),
            Some(ControlEvent::Jog { steps: 3 })
        );
        for controller in [24u8, 47, 61] {
            let message = MidiMessage::ControlChange {
                channel: 0,
                controller,
                value: 0x03,
            };
            assert_eq!(ControlEvent::from_midi(&X_TOUCH, &message), None);
        }
    }

    #[test]
    fn a_fader_move_is_addressed_by_pitch_bend_channel() {
        for strip in 0..8u8 {
            let message = MidiMessage::PitchBend {
                channel: strip,
                value: 9000 + u16::from(strip),
            };
            assert_eq!(
                ControlEvent::from_midi(&X_TOUCH, &message),
                Some(ControlEvent::Move {
                    fader: Fader::Strip(strip),
                    position: 9000 + u16::from(strip)
                })
            );
        }
        let main = MidiMessage::PitchBend {
            channel: 8,
            value: 12_700,
        };
        assert_eq!(
            ControlEvent::from_midi(&X_TOUCH, &main),
            Some(ControlEvent::Move {
                fader: Fader::Main,
                position: 12_700
            })
        );
        let ninth = MidiMessage::PitchBend {
            channel: 9,
            value: 1,
        };
        assert_eq!(ControlEvent::from_midi(&X_TOUCH, &ninth), None);
    }

    #[test]
    fn a_fader_touch_is_not_a_button_press() {
        // Notes 104-112 sit immediately above the global button block, and
        // reporting them as buttons would make touching a fader press
        // something.
        let message = MidiMessage::NoteOn {
            channel: 0,
            note: 106,
            velocity: 0x7F,
        };
        assert_eq!(
            ControlEvent::from_midi(&X_TOUCH, &message),
            Some(ControlEvent::Touch {
                fader: Fader::Strip(2),
                touched: true
            })
        );
        assert_eq!(
            ControlEvent::Touch {
                fader: Fader::Strip(2),
                touched: true
            }
            .to_midi(&X_TOUCH),
            Ok(message)
        );
    }

    #[test]
    fn a_message_on_another_channel_is_not_this_surface_speaking() {
        // Two surfaces on one port is the reason: an extender's Select must not
        // arrive as this desk's.
        for message in [
            MidiMessage::NoteOn {
                channel: 1,
                note: 24,
                velocity: 0x7F,
            },
            MidiMessage::NoteOff {
                channel: 2,
                note: 24,
                velocity: 0,
            },
            MidiMessage::ControlChange {
                channel: 3,
                controller: 16,
                value: 1,
            },
        ] {
            assert_eq!(ControlEvent::from_midi(&X_TOUCH, &message), None);
        }
    }

    #[test]
    fn a_message_the_profile_does_not_describe_produces_nothing() {
        for message in [
            MidiMessage::NoteOn {
                channel: 0,
                note: 120,
                velocity: 0x7F,
            },
            MidiMessage::ProgramChange {
                channel: 0,
                program: 3,
            },
            MidiMessage::ChannelPressure {
                channel: 0,
                pressure: 0x21,
            },
            MidiMessage::PolyPressure {
                channel: 0,
                note: 24,
                pressure: 0x21,
            },
            MidiMessage::SysEx(&[0x00, 0x00, 0x66, 0x14, 0x01]),
        ] {
            assert_eq!(ControlEvent::from_midi(&X_TOUCH, &message), None);
        }
    }

    #[test]
    fn a_control_this_surface_does_not_have_cannot_be_written() {
        // The X-Touch has eight strips. An executor page that ran off the end
        // must produce an error rather than note 8, which belongs to strip 0's
        // Solo.
        assert_eq!(
            ControlEvent::Button {
                button: ButtonId::Strip {
                    strip: 8,
                    button: StripButton::Rec
                },
                pressed: true
            }
            .to_midi(&X_TOUCH),
            Err(EncodeError::NoSuchControl)
        );
        assert_eq!(
            ControlEvent::Touch {
                fader: Fader::Strip(9),
                touched: true
            }
            .to_midi(&X_TOUCH),
            Err(EncodeError::NoSuchControl)
        );
        assert_eq!(
            ControlEvent::Move {
                fader: Fader::Strip(9),
                position: 1
            }
            .to_midi(&X_TOUCH),
            Err(EncodeError::NoSuchControl)
        );
        assert_eq!(
            ControlEvent::VPot { strip: 8, steps: 1 }.to_midi(&X_TOUCH),
            Err(EncodeError::NoSuchControl)
        );
    }

    #[test]
    fn a_fader_position_wider_than_fourteen_bits_is_refused() {
        assert_eq!(
            ControlEvent::Move {
                fader: Fader::Main,
                position: FADER_MAX + 1
            }
            .to_midi(&X_TOUCH),
            Err(EncodeError::OutOfRange {
                field: "fader position",
                value: 0x4000
            })
        );
        assert!(
            ControlEvent::Move {
                fader: Fader::Main,
                position: FADER_MAX
            }
            .to_midi(&X_TOUCH)
            .is_ok()
        );
    }

    #[test]
    fn an_event_writes_itself_into_a_caller_buffer() {
        let mut buf = [0u8; 3];
        let written = ControlEvent::Button {
            button: ButtonId::Global(GlobalButton::Flip),
            pressed: true,
        }
        .encode_into(&X_TOUCH, &mut buf)
        .expect("three bytes is enough");
        assert_eq!(&buf[..written], &[0x90, 50, 0x7F]);

        let mut small = [0u8; 2];
        assert_eq!(
            ControlEvent::Jog { steps: 1 }.encode_into(&X_TOUCH, &mut small),
            Err(EncodeError::BufferTooSmall {
                needed: 3,
                offered: 2
            })
        );
    }

    #[test]
    fn a_button_names_itself_the_way_the_surface_model_will() {
        // docs/MCU_MAPPING.md §3 spells the logical control set this way, and a
        // log line an operator reads should match the document they were given.
        assert_eq!(
            ButtonId::Strip {
                strip: 4,
                button: StripButton::Mute
            }
            .to_string(),
            "Strip[4].Button.Mute"
        );
        assert_eq!(
            ButtonId::Global(GlobalButton::BankLeft).to_string(),
            "Global.BankLeft"
        );
    }
}
