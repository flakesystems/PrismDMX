//! The inbound front door: bytes from a MIDI port, control events out.
//!
//! [`McuCodec`] is a [`MidiDecoder`] with a [`McuProfile`] beside it and two
//! more counters. It is what the `midi-in` thread of `ARCHITECTURE_SPEC.md` §3
//! will hold, and it is the whole of layer 1's inbound half: no domain types,
//! no session, no `Command`. Turning a [`ControlEvent`] into something PrismDMX
//! does is layer 3, and it is S22's.
//!
//! # Inbound SysEx is counted, not interpreted
//!
//! The MCU handshake — the host's *are you there* and the surface's *Device
//! Ready* — is **optional**. When S19 wrote this codec no source settled what
//! the answer's payload looked like on an X-Touch, so rather than invent data for
//! a message shape nobody had seen, it counts an inbound Mackie SysEx
//! (`sysex_ignored`) and hands it on to nobody. The *question* is written —
//! [`Feedback::DeviceQuery`](crate::Feedback::DeviceQuery) — and the reassembly,
//! the bounded buffer and the timeout are all exercised by it.
//!
//! **S20 asked the desk, and `docs/MCU_MAPPING.md` §2.3 now has the answer**:
//! `F0 00 00 66 14 01` followed by eleven printable ASCII bytes — a
//! seven-character serial and a four-character challenge — plus an undocumented
//! firmware version request beside it. So a decoder is writeable now. It is still
//! not written, because nothing above layer 1 has asked for a serial number, and
//! an API invented for no caller is a guess of a different kind. What was missing
//! was the bytes; the bytes are written down.

use std::time::Duration;

use crate::control::ControlEvent;
use crate::midi::{DecodeCounters, MidiDecoder, MidiMessage};
use crate::profile::McuProfile;

/// Everything the codec has counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CodecCounters {
    /// What the MIDI wire layer saw.
    pub wire: DecodeCounters,
    /// Well-formed MIDI messages that this surface's profile does not describe
    /// — a note outside every table, a CC that is not a V-Pot or the jog wheel,
    /// a message on somebody else's channel.
    pub unmapped: u64,
    /// Complete System Exclusive messages, which this layer does not turn into
    /// control events. See the module documentation.
    pub sysex_ignored: u64,
}

impl CodecCounters {
    /// Everything that arrived intact and produced no event.
    #[must_use]
    pub const fn ignored(&self) -> u64 {
        self.unmapped + self.sysex_ignored
    }

    /// Everything thrown away, malformed or merely unrecognised.
    #[must_use]
    pub const fn discarded(&self) -> u64 {
        self.wire.discarded() + self.ignored()
    }
}

/// A Mackie Control codec for one surface.
#[derive(Debug)]
pub struct McuCodec {
    profile: McuProfile,
    decoder: MidiDecoder,
    unmapped: u64,
    sysex_ignored: u64,
}

impl McuCodec {
    /// A codec for a surface, with the default SysEx timeout.
    #[must_use]
    pub const fn new(profile: McuProfile) -> Self {
        Self {
            profile,
            decoder: MidiDecoder::new(),
            unmapped: 0,
            sysex_ignored: 0,
        }
    }

    /// A codec that drops an unterminated SysEx after `timeout`.
    #[must_use]
    pub const fn with_timeout(profile: McuProfile, timeout: Duration) -> Self {
        Self {
            profile,
            decoder: MidiDecoder::with_timeout(timeout),
            unmapped: 0,
            sysex_ignored: 0,
        }
    }

    /// The surface this codec speaks to.
    #[must_use]
    pub const fn profile(&self) -> &McuProfile {
        &self.profile
    }

    /// What has been decoded and what has been thrown away.
    #[must_use]
    pub const fn counters(&self) -> CodecCounters {
        CodecCounters {
            wire: self.decoder.counters(),
            unmapped: self.unmapped,
            sysex_ignored: self.sysex_ignored,
        }
    }

    /// Drops a half-arrived SysEx that has aged out. See
    /// [`MidiDecoder::poll`].
    pub fn poll(&mut self, now: Duration) -> bool {
        self.decoder.poll(now)
    }

    /// Feeds bytes from the port, calling `sink` once per control event.
    ///
    /// `now` is the arrival time of this packet on any monotonic scale.
    /// Allocates nothing, whatever the bytes are.
    pub fn push<F>(&mut self, bytes: &[u8], now: Duration, mut sink: F)
    where
        F: FnMut(ControlEvent),
    {
        // Bound each field separately so the closure borrows the counters while
        // the decoder is borrowed mutably. The profile is `Copy`, so it comes
        // along by value.
        let profile = self.profile;
        let unmapped = &mut self.unmapped;
        let sysex_ignored = &mut self.sysex_ignored;
        self.decoder.push(bytes, now, |message| {
            match ControlEvent::from_midi(&profile, &message) {
                Some(event) => sink(event),
                None if matches!(message, MidiMessage::SysEx(_)) => *sysex_ignored += 1,
                None => *unmapped += 1,
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::McuCodec;
    use crate::control::{ButtonId, ControlEvent};
    use crate::feedback::{Feedback, StripColor};
    use crate::midi::MAX_MESSAGE_BYTES;
    use crate::profile::{Fader, GlobalButton, StripButton, X_TOUCH};
    use std::time::Duration;

    fn events(codec: &mut McuCodec, bytes: &[u8]) -> Vec<ControlEvent> {
        let mut out = Vec::new();
        codec.push(bytes, Duration::from_millis(1), |event| out.push(event));
        out
    }

    #[test]
    fn a_burst_from_the_surface_becomes_the_events_it_describes() {
        // One packet holding a press, a release, a fader move, a V-Pot turn
        // and a touch - which is what a hand on the desk produces, rather than
        // one message at a time.
        let mut codec = McuCodec::new(X_TOUCH);
        let out = events(
            &mut codec,
            &[
                0x90, 26, 0x7F, // Select on strip 3, down
                0x90, 26, 0x00, // and up
                0xE4, 0x0C, 0x63, // strip 5 fader
                0xB0, 21, 0x42, // strip 6 V-Pot, two back
                0x90, 108, 0x7F, // strip 5 fader touched
            ],
        );
        assert_eq!(
            out,
            vec![
                ControlEvent::Button {
                    button: ButtonId::Strip {
                        strip: 2,
                        button: StripButton::Select
                    },
                    pressed: true
                },
                ControlEvent::Button {
                    button: ButtonId::Strip {
                        strip: 2,
                        button: StripButton::Select
                    },
                    pressed: false
                },
                ControlEvent::Move {
                    fader: Fader::Strip(4),
                    position: 0x0C | (0x63 << 7)
                },
                ControlEvent::VPot {
                    strip: 5,
                    steps: -2
                },
                ControlEvent::Touch {
                    fader: Fader::Strip(4),
                    touched: true
                },
            ]
        );
        assert_eq!(codec.counters().discarded(), 0);
        assert_eq!(codec.counters().wire.messages, 5);
    }

    #[test]
    fn a_well_formed_message_the_profile_does_not_know_is_counted_as_unmapped() {
        // Distinct from a malformed one on purpose: a surface sending notes
        // nobody has mapped is a profile that is wrong, and a cable dropping
        // bytes is a cable. The two need different counters or the same number
        // means both.
        let mut codec = McuCodec::new(X_TOUCH);
        let out = events(&mut codec, &[0x90, 120, 0x7F, 0xB0, 100, 0x01]);
        assert!(out.is_empty());
        assert_eq!(codec.counters().unmapped, 2);
        assert_eq!(codec.counters().wire.discarded(), 0);
        assert_eq!(codec.counters().wire.messages, 2);
    }

    #[test]
    fn an_inbound_sysex_is_counted_rather_than_guessed_at() {
        // The reassembly, the bounded buffer and the timeout all still run -
        // what does not happen is a control event invented from a message shape
        // no source could settle. See the module documentation.
        let mut codec = McuCodec::new(X_TOUCH);
        let mut buf = [0u8; MAX_MESSAGE_BYTES];
        let written = Feedback::DisplayColors([StripColor::Cyan; 8])
            .encode_into(&X_TOUCH, &mut buf)
            .expect("the colour message fits");
        let out = events(&mut codec, buf.get(..written).expect("written fits"));
        assert!(out.is_empty());
        assert_eq!(codec.counters().sysex_ignored, 1);
        assert_eq!(codec.counters().unmapped, 0);
        assert_eq!(codec.counters().ignored(), 1);
    }

    #[test]
    fn the_codec_keeps_its_place_when_a_packet_is_rubbish() {
        // CLAUDE.md: an invalid MIDI packet must never propagate a failure. The
        // stronger claim, and the one that matters on a desk, is that the
        // *next* press still arrives.
        let mut codec = McuCodec::new(X_TOUCH);
        let out = events(
            &mut codec,
            &[
                0x42, 0x11, // orphan data
                0x90, 0x18, // a press cut short
                0xF0, 0x00, 0x00, // a sysex that never ends...
                0x90, 94, 0x7F, // ...and the Play button, which must arrive
            ],
        );
        assert_eq!(
            out,
            vec![ControlEvent::Button {
                button: ButtonId::Global(GlobalButton::Play),
                pressed: true
            }]
        );
        let counters = codec.counters();
        assert_eq!(counters.wire.orphan_data, 2);
        assert_eq!(counters.wire.truncated, 1);
        assert_eq!(counters.wire.sysex_interrupted, 1);
        assert_eq!(counters.discarded(), 4);
    }

    #[test]
    fn the_codec_forwards_the_sysex_timeout_and_names_its_surface() {
        let mut codec = McuCodec::with_timeout(X_TOUCH, Duration::from_millis(10));
        codec.push(&[0xF0, 0x00], Duration::ZERO, |_| {
            panic!("half a sysex is not an event")
        });
        assert!(!codec.poll(Duration::from_millis(5)));
        assert!(codec.poll(Duration::from_millis(10)));
        assert_eq!(codec.counters().wire.sysex_timeout, 1);
        assert_eq!(codec.profile().name, "Behringer X-Touch (MC mode)");
        assert!(codec.profile().verified);
    }

    #[test]
    fn the_counters_start_empty() {
        let codec = McuCodec::new(X_TOUCH);
        assert_eq!(codec.counters(), Default::default());
        assert_eq!(codec.counters().discarded(), 0);
        assert_eq!(codec.counters().ignored(), 0);
    }
}
