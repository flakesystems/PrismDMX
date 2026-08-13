//! The round trip, in both directions, on the bytes.
//!
//! `IMPLEMENTATION_PLAN.md` S19, first exit criterion, and
//! `docs/MCU_MAPPING.md` §6: *table-driven — MIDI bytes → control event → MIDI
//! bytes, asserting byte equality both ways*.
//!
//! # Why the tables are written out by hand
//!
//! S16 learned this the expensive way: `to_vec` and `to_vec_named` both produce
//! something that reads back, and only an assertion about the **bytes** could
//! see the difference. A round-trip test that computed its expected bytes from
//! the same profile the codec uses would be a function composed with its own
//! inverse — it would pass with every note number in the table shifted by one.
//!
//! So [`INBOUND`] and [`OUTBOUND`] hold literal byte arrays and literal note
//! numbers, transcribed from `docs/MCU_MAPPING.md` §2 rather than derived from
//! [`X_TOUCH`]. When S20 finds a row is wrong, two places change and the second
//! one is this file — which is the point: a table that agreed with itself
//! whatever it said would make the verification session meaningless.
//!
//! # Canonical rows and alias rows
//!
//! Some byte sequences decode to an event that does **not** write back as those
//! bytes, and that is correct rather than a defect. A real Note Off means a
//! release, and a release is written as a Note On with velocity 0 because that
//! is what the surface sends. Those rows live in [`INBOUND_ALIASES`] with the
//! canonical form they normalise onto, so the difference is asserted instead of
//! being quietly excluded from the table.

use std::time::Duration;

use proptest::prelude::*;

use prism_surface::{
    ButtonId, ControlEvent, FADER_MAX, Fader, Feedback, GlobalButton, LcdMeterMode, LedState,
    MAX_MESSAGE_BYTES, McuCodec, MeterSignal, MidiDecoder, RingMode, SCRIBBLE_STRIP_COLORS,
    SegmentChar, StripButton, StripColor, VPotRing, X_TOUCH,
};

/// A moment. Nothing in these tests depends on it — no message here is a SysEx
/// that could age out — but the decoder takes one, so it is named rather than
/// scattered.
const NOW: Duration = Duration::from_millis(7);

/// One inbound row: what the surface sends, and what it means.
struct Inbound {
    what: &'static str,
    bytes: &'static [u8],
    event: ControlEvent,
}

/// Bytes the X-Touch sends, transcribed from `docs/MCU_MAPPING.md` §2.1.
///
/// At least one row from every group of the document, and the numbers are
/// written out: `0x90` is note-on on channel 1, `24 + n` is strip *n*'s Select.
const INBOUND: &[Inbound] = &[
    Inbound {
        what: "strip 1 Rec pressed",
        bytes: &[0x90, 0, 0x7F],
        event: ControlEvent::Button {
            button: ButtonId::Strip {
                strip: 0,
                button: StripButton::Rec,
            },
            pressed: true,
        },
    },
    Inbound {
        what: "strip 8 Rec released",
        bytes: &[0x90, 7, 0x00],
        event: ControlEvent::Button {
            button: ButtonId::Strip {
                strip: 7,
                button: StripButton::Rec,
            },
            pressed: false,
        },
    },
    Inbound {
        what: "strip 4 Solo pressed",
        bytes: &[0x90, 11, 0x7F],
        event: ControlEvent::Button {
            button: ButtonId::Strip {
                strip: 3,
                button: StripButton::Solo,
            },
            pressed: true,
        },
    },
    Inbound {
        what: "strip 6 Mute pressed",
        bytes: &[0x90, 21, 0x7F],
        event: ControlEvent::Button {
            button: ButtonId::Strip {
                strip: 5,
                button: StripButton::Mute,
            },
            pressed: true,
        },
    },
    Inbound {
        what: "strip 3 Select pressed",
        bytes: &[0x90, 26, 0x7F],
        event: ControlEvent::Button {
            button: ButtonId::Strip {
                strip: 2,
                button: StripButton::Select,
            },
            pressed: true,
        },
    },
    Inbound {
        what: "strip 7 V-Pot pushed",
        bytes: &[0x90, 38, 0x7F],
        event: ControlEvent::Button {
            button: ButtonId::Strip {
                strip: 6,
                button: StripButton::VPotPush,
            },
            pressed: true,
        },
    },
    Inbound {
        what: "Encoder Assign: Pan/Surround",
        bytes: &[0x90, 42, 0x7F],
        event: ControlEvent::Button {
            button: ButtonId::Global(GlobalButton::AssignPan),
            pressed: true,
        },
    },
    Inbound {
        what: "Bank left — executor page down (D7)",
        bytes: &[0x90, 46, 0x7F],
        event: ControlEvent::Button {
            button: ButtonId::Global(GlobalButton::BankLeft),
            pressed: true,
        },
    },
    Inbound {
        what: "Channel right — next UI view (D8)",
        bytes: &[0x90, 49, 0x7F],
        event: ControlEvent::Button {
            button: ButtonId::Global(GlobalButton::ChannelRight),
            pressed: true,
        },
    },
    Inbound {
        what: "Flip",
        bytes: &[0x90, 50, 0x7F],
        event: ControlEvent::Button {
            button: ButtonId::Global(GlobalButton::Flip),
            pressed: true,
        },
    },
    Inbound {
        what: "Name/Value",
        bytes: &[0x90, 52, 0x7F],
        event: ControlEvent::Button {
            button: ButtonId::Global(GlobalButton::NameValue),
            pressed: true,
        },
    },
    Inbound {
        what: "F1 — an XKey",
        bytes: &[0x90, 54, 0x7F],
        event: ControlEvent::Button {
            button: ButtonId::Global(GlobalButton::F1),
            pressed: true,
        },
    },
    Inbound {
        what: "F8 — the last XKey",
        bytes: &[0x90, 61, 0x7F],
        event: ControlEvent::Button {
            button: ButtonId::Global(GlobalButton::F8),
            pressed: true,
        },
    },
    Inbound {
        what: "Global View group: Aux",
        bytes: &[0x90, 66, 0x7F],
        event: ControlEvent::Button {
            button: ButtonId::Global(GlobalButton::ViewAux),
            pressed: true,
        },
    },
    Inbound {
        what: "Shift held",
        bytes: &[0x90, 70, 0x7F],
        event: ControlEvent::Button {
            button: ButtonId::Global(GlobalButton::ModShift),
            pressed: true,
        },
    },
    Inbound {
        what: "Shift let go",
        bytes: &[0x90, 70, 0x00],
        event: ControlEvent::Button {
            button: ButtonId::Global(GlobalButton::ModShift),
            pressed: false,
        },
    },
    Inbound {
        what: "Automation: Latch",
        bytes: &[0x90, 78, 0x7F],
        event: ControlEvent::Button {
            button: ButtonId::Global(GlobalButton::AutoLatch),
            pressed: true,
        },
    },
    Inbound {
        what: "Undo — the Oops button",
        bytes: &[0x90, 81, 0x7F],
        event: ControlEvent::Button {
            button: ButtonId::Global(GlobalButton::Undo),
            pressed: true,
        },
    },
    Inbound {
        what: "Cycle",
        bytes: &[0x90, 86, 0x7F],
        event: ControlEvent::Button {
            button: ButtonId::Global(GlobalButton::Cycle),
            pressed: true,
        },
    },
    Inbound {
        what: "Play",
        bytes: &[0x90, 94, 0x7F],
        event: ControlEvent::Button {
            button: ButtonId::Global(GlobalButton::Play),
            pressed: true,
        },
    },
    Inbound {
        what: "Record — the three-stage Clear",
        bytes: &[0x90, 95, 0x7F],
        event: ControlEvent::Button {
            button: ButtonId::Global(GlobalButton::Record),
            pressed: true,
        },
    },
    Inbound {
        what: "Zoom ▲ — programmer page up",
        bytes: &[0x90, 96, 0x7F],
        event: ControlEvent::Button {
            button: ButtonId::Global(GlobalButton::CursorUp),
            pressed: true,
        },
    },
    Inbound {
        what: "Scrub",
        bytes: &[0x90, 101, 0x7F],
        event: ControlEvent::Button {
            button: ButtonId::Global(GlobalButton::Scrub),
            pressed: true,
        },
    },
    Inbound {
        what: "foot switch 2",
        bytes: &[0x90, 103, 0x7F],
        event: ControlEvent::Button {
            button: ButtonId::Global(GlobalButton::FootSwitch2),
            pressed: true,
        },
    },
    Inbound {
        what: "strip 1 fader touched",
        bytes: &[0x90, 104, 0x7F],
        event: ControlEvent::Touch {
            fader: Fader::Strip(0),
            touched: true,
        },
    },
    Inbound {
        what: "strip 8 fader released",
        bytes: &[0x90, 111, 0x00],
        event: ControlEvent::Touch {
            fader: Fader::Strip(7),
            touched: false,
        },
    },
    Inbound {
        what: "main fader touched",
        bytes: &[0x90, 112, 0x7F],
        event: ControlEvent::Touch {
            fader: Fader::Main,
            touched: true,
        },
    },
    Inbound {
        what: "strip 1 fader at the bottom",
        bytes: &[0xE0, 0x00, 0x00],
        event: ControlEvent::Move {
            fader: Fader::Strip(0),
            position: 0,
        },
    },
    Inbound {
        what: "strip 3 fader, LSB first",
        bytes: &[0xE2, 0x6C, 0x63],
        event: ControlEvent::Move {
            fader: Fader::Strip(2),
            position: 12_780,
        },
    },
    Inbound {
        what: "strip 8 fader at the top",
        bytes: &[0xE7, 0x7F, 0x7F],
        event: ControlEvent::Move {
            fader: Fader::Strip(7),
            position: 16_383,
        },
    },
    Inbound {
        what: "main fader at the 0 dB detent — 12 700 of 16 383",
        bytes: &[0xE8, 0x1C, 0x63],
        event: ControlEvent::Move {
            fader: Fader::Main,
            position: 12_700,
        },
    },
    Inbound {
        what: "strip 1 V-Pot one detent clockwise",
        bytes: &[0xB0, 16, 0x01],
        event: ControlEvent::VPot { strip: 0, steps: 1 },
    },
    Inbound {
        what: "strip 1 V-Pot one detent back",
        bytes: &[0xB0, 16, 0x41],
        event: ControlEvent::VPot {
            strip: 0,
            steps: -1,
        },
    },
    Inbound {
        what: "strip 8 V-Pot spun hard — magnitude unconfirmed, §7 measures it",
        bytes: &[0xB0, 23, 0x0B],
        event: ControlEvent::VPot {
            strip: 7,
            steps: 11,
        },
    },
    Inbound {
        what: "jog wheel forward",
        bytes: &[0xB0, 60, 0x02],
        event: ControlEvent::Jog { steps: 2 },
    },
    Inbound {
        what: "jog wheel back",
        bytes: &[0xB0, 60, 0x43],
        event: ControlEvent::Jog { steps: -3 },
    },
];

/// One inbound row that decodes correctly but writes back as something else.
struct InboundAlias {
    what: &'static str,
    bytes: &'static [u8],
    canonical: &'static [u8],
}

/// Byte sequences a device may send that normalise onto a canonical form.
const INBOUND_ALIASES: &[InboundAlias] = &[
    InboundAlias {
        what: "a real Note Off is a release",
        bytes: &[0x80, 26, 0x40],
        canonical: &[0x90, 26, 0x00],
    },
    InboundAlias {
        what: "any non-zero velocity is a press",
        bytes: &[0x90, 26, 0x01],
        canonical: &[0x90, 26, 0x7F],
    },
    InboundAlias {
        what: "minus zero on a V-Pot is no movement",
        bytes: &[0xB0, 18, 0x40],
        canonical: &[0xB0, 18, 0x00],
    },
];

/// One outbound row: what the desk shows, and the bytes that show it.
struct Outbound {
    what: &'static str,
    bytes: &'static [u8],
    feedback: Feedback<'static>,
}

/// Bytes PrismDMX sends, transcribed from `docs/MCU_MAPPING.md` §2.2 and §2.3.
const OUTBOUND: &[Outbound] = &[
    Outbound {
        what: "strip 2 Select lit",
        bytes: &[0x90, 25, 0x7F],
        feedback: Feedback::Led {
            button: ButtonId::Strip {
                strip: 1,
                button: StripButton::Select,
            },
            state: LedState::On,
        },
    },
    Outbound {
        what: "strip 2 Select dark",
        bytes: &[0x90, 25, 0x00],
        feedback: Feedback::Led {
            button: ButtonId::Strip {
                strip: 1,
                button: StripButton::Select,
            },
            state: LedState::Off,
        },
    },
    Outbound {
        what: "Save flashing — unsaved changes",
        bytes: &[0x90, 80, 0x01],
        feedback: Feedback::Led {
            button: ButtonId::Global(GlobalButton::Save),
            state: LedState::Flashing,
        },
    },
    Outbound {
        what: "strip 5 motor fader to three quarters",
        bytes: &[0xE4, 0x40, 0x60],
        feedback: Feedback::Move {
            fader: Fader::Strip(4),
            position: 12_352,
        },
    },
    Outbound {
        what: "main motor fader to the top",
        bytes: &[0xE8, 0x7F, 0x7F],
        feedback: Feedback::Move {
            fader: Fader::Main,
            position: 16_383,
        },
    },
    Outbound {
        what: "strip 1 ring: single dot at 6, lamp off",
        bytes: &[0xB0, 48, 0x06],
        feedback: Feedback::Ring {
            strip: 0,
            ring: VPotRing {
                mode: RingMode::Dot,
                position: 6,
                led: false,
            },
        },
    },
    Outbound {
        what: "strip 4 ring: wrap to 11, lamp on",
        bytes: &[0xB0, 51, 0x6B],
        feedback: Feedback::Ring {
            strip: 3,
            ring: VPotRing {
                mode: RingMode::Wrap,
                position: 11,
                led: true,
            },
        },
    },
    Outbound {
        what: "strip 8 ring: spread of 5",
        bytes: &[0xB0, 55, 0x35],
        feedback: Feedback::Ring {
            strip: 7,
            ring: VPotRing {
                mode: RingMode::Spread,
                position: 5,
                led: false,
            },
        },
    },
    Outbound {
        what: "strip 3 meter at 0 dB",
        bytes: &[0xD0, 0x2C],
        feedback: Feedback::Meter {
            strip: 2,
            signal: MeterSignal::Level(12),
        },
    },
    Outbound {
        what: "strip 6 overload marker set",
        bytes: &[0xD0, 0x5E],
        feedback: Feedback::Meter {
            strip: 5,
            signal: MeterSignal::Overload(true),
        },
    },
    Outbound {
        what: "rightmost 7-segment digit shows 4",
        bytes: &[0xB0, 64, 52],
        feedback: Feedback::Segment {
            digit: 0,
            character: SegmentChar {
                code: 52,
                dot: false,
            },
        },
    },
    Outbound {
        what: "third digit shows 9 with its dot",
        bytes: &[0xB0, 66, 0x79],
        feedback: Feedback::Segment {
            digit: 2,
            character: SegmentChar {
                code: 57,
                dot: true,
            },
        },
    },
    Outbound {
        what: "the assignment display's left digit shows P",
        bytes: &[0xB0, 75, 16],
        feedback: Feedback::Segment {
            digit: 11,
            character: SegmentChar {
                code: 16,
                dot: false,
            },
        },
    },
    Outbound {
        what: "strip 1's upper scribble line",
        bytes: &[
            0xF0, 0x00, 0x00, 0x66, 0x14, 0x12, 0x00, b'S', b'P', b'O', b'T', b' ', b'1', 0xF7,
        ],
        feedback: Feedback::DisplayText {
            offset: 0x00,
            text: b"SPOT 1",
        },
    },
    Outbound {
        what: "strip 4's lower scribble line — an offset, not another message",
        bytes: &[
            0xF0, 0x00, 0x00, 0x66, 0x14, 0x12, 0x4D, b'4', b'2', b'%', 0xF7,
        ],
        feedback: Feedback::DisplayText {
            offset: 0x38 + 21,
            text: b"42%",
        },
    },
    Outbound {
        what: "the colour row — the extension this device is bought for",
        bytes: &[
            0xF0, 0x00, 0x00, 0x66, 0x14, 0x72, 7, 1, 3, 2, 6, 4, 5, 0, 0xF7,
        ],
        feedback: Feedback::DisplayColors([
            StripColor::White,
            StripColor::Red,
            StripColor::Yellow,
            StripColor::Green,
            StripColor::Cyan,
            StripColor::Blue,
            StripColor::Magenta,
            StripColor::Off,
        ]),
    },
    Outbound {
        what: "LCD meters horizontal",
        bytes: &[0xF0, 0x00, 0x00, 0x66, 0x14, 0x21, 0x00, 0xF7],
        feedback: Feedback::MeterMode(LcdMeterMode::Horizontal),
    },
    Outbound {
        what: "the optional handshake question",
        bytes: &[0xF0, 0x00, 0x00, 0x66, 0x14, 0x00, 0xF7],
        feedback: Feedback::DeviceQuery,
    },
];

/// Decodes a byte stream into control events, through the whole inbound path.
fn decode_events(bytes: &[u8]) -> Vec<ControlEvent> {
    let mut codec = McuCodec::new(X_TOUCH);
    let mut out = Vec::new();
    codec.push(bytes, NOW, |event| out.push(event));
    out
}

/// Writes one event, through the whole outbound path.
fn encode_event(event: &ControlEvent) -> Vec<u8> {
    let mut buf = [0u8; MAX_MESSAGE_BYTES];
    let written = event
        .encode_into(&X_TOUCH, &mut buf)
        .expect("a control the surface has");
    buf[..written].to_vec()
}

/// Decodes a byte stream into feedback messages, reassembly included.
fn decode_feedback(bytes: &[u8]) -> Vec<Feedback<'static>> {
    let mut decoder = MidiDecoder::new();
    let mut out = Vec::new();
    decoder.push(bytes, NOW, |message| {
        // The payload borrow ends with this call, and every `Feedback` in the
        // tables below either owns its bytes or points at a `'static` literal,
        // so what comes out is copied into an owning form here.
        if let Some(feedback) = Feedback::from_midi(&X_TOUCH, &message) {
            out.push(own(feedback));
        }
    });
    out
}

/// Copies a decoded [`Feedback`] out of the decoder's buffer.
///
/// Only [`Feedback::DisplayText`] borrows, and the tables hold `'static` text,
/// so this is a lookup rather than a copy — but it has to be written out,
/// because a decoded message may not outlive the sink call.
fn own(feedback: Feedback<'_>) -> Feedback<'static> {
    match feedback {
        Feedback::DisplayText { offset, text } => {
            let found = OUTBOUND.iter().find_map(|row| match row.feedback {
                Feedback::DisplayText {
                    offset: want,
                    text: known,
                } if want == offset && known == text => Some(known),
                _ => None,
            });
            Feedback::DisplayText {
                offset,
                text: found.unwrap_or(b""),
            }
        }
        Feedback::Led { button, state } => Feedback::Led { button, state },
        Feedback::Move { fader, position } => Feedback::Move { fader, position },
        Feedback::Ring { strip, ring } => Feedback::Ring { strip, ring },
        Feedback::Meter { strip, signal } => Feedback::Meter { strip, signal },
        Feedback::Segment { digit, character } => Feedback::Segment { digit, character },
        Feedback::DisplayColors(colors) => Feedback::DisplayColors(colors),
        Feedback::MeterMode(mode) => Feedback::MeterMode(mode),
        Feedback::DeviceQuery => Feedback::DeviceQuery,
    }
}

/// Writes one feedback message.
fn encode_feedback(feedback: &Feedback<'_>) -> Vec<u8> {
    let mut buf = [0u8; MAX_MESSAGE_BYTES];
    let written = feedback
        .encode_into(&X_TOUCH, &mut buf)
        .expect("a control the surface has");
    buf[..written].to_vec()
}

#[test]
fn every_inbound_row_reads_as_its_event_and_writes_back_as_its_bytes() {
    // Both directions on every row, which is the criterion. The bytes are the
    // ones written out at the top of this file, so an agreement here is an
    // agreement with docs/MCU_MAPPING.md §2.1 rather than with the profile.
    for row in INBOUND {
        assert_eq!(
            decode_events(row.bytes),
            vec![row.event],
            "decoding {}: {:02X?}",
            row.what,
            row.bytes
        );
        assert_eq!(encode_event(&row.event), row.bytes, "encoding {}", row.what);
    }
}

#[test]
fn the_inbound_table_covers_every_kind_of_control_the_surface_has() {
    // A table is only as good as what is in it, and the failure mode of a
    // hand-written one is a whole group nobody transcribed.
    let mut kinds = [false; 5];
    for row in INBOUND {
        let index = match row.event {
            ControlEvent::Button { .. } => 0,
            ControlEvent::Touch { .. } => 1,
            ControlEvent::Move { .. } => 2,
            ControlEvent::VPot { .. } => 3,
            ControlEvent::Jog { .. } => 4,
        };
        if let Some(slot) = kinds.get_mut(index) {
            *slot = true;
        }
    }
    assert!(kinds.into_iter().all(|seen| seen), "{kinds:?}");

    // Both ends of every range that has one: strip 1 and strip 8, the main
    // fader, a press and a release, a positive and a negative detent.
    let has = |event: ControlEvent| INBOUND.iter().any(|row| row.event == event);
    assert!(has(ControlEvent::Touch {
        fader: Fader::Main,
        touched: true
    }));
    assert!(has(ControlEvent::Move {
        fader: Fader::Strip(7),
        position: 16_383
    }));
    assert!(has(ControlEvent::VPot {
        strip: 0,
        steps: -1
    }));
    assert!(has(ControlEvent::Jog { steps: -3 }));
}

#[test]
fn an_alias_decodes_to_the_same_event_and_writes_back_as_the_canonical_form() {
    // The rows where byte equality deliberately does not hold, and the reason
    // it does not: the surface has one way of saying a release and we write
    // that one, whatever a device sends us.
    for row in INBOUND_ALIASES {
        let alias = decode_events(row.bytes);
        let canonical = decode_events(row.canonical);
        assert_eq!(alias, canonical, "{}", row.what);
        assert_eq!(alias.len(), 1, "{}", row.what);
        let event = alias.first().expect("one event");
        assert_eq!(encode_event(event), row.canonical, "{}", row.what);
        assert_ne!(row.bytes, row.canonical, "{} is not an alias", row.what);
    }
}

#[test]
fn every_outbound_row_writes_its_bytes_and_reads_back_as_itself() {
    for row in OUTBOUND {
        assert_eq!(
            encode_feedback(&row.feedback),
            row.bytes,
            "encoding {}",
            row.what
        );
        assert_eq!(
            decode_feedback(row.bytes),
            vec![row.feedback],
            "decoding {}: {:02X?}",
            row.what,
            row.bytes
        );
    }
}

#[test]
fn the_outbound_table_covers_every_kind_of_message_the_desk_sends() {
    let mut kinds = [false; 9];
    for row in OUTBOUND {
        let index = match row.feedback {
            Feedback::Led { .. } => 0,
            Feedback::Move { .. } => 1,
            Feedback::Ring { .. } => 2,
            Feedback::Meter { .. } => 3,
            Feedback::Segment { .. } => 4,
            Feedback::DisplayText { .. } => 5,
            Feedback::DisplayColors(_) => 6,
            Feedback::MeterMode(_) => 7,
            Feedback::DeviceQuery => 8,
        };
        if let Some(slot) = kinds.get_mut(index) {
            *slot = true;
        }
    }
    assert!(kinds.into_iter().all(|seen| seen), "{kinds:?}");
}

#[test]
fn a_whole_burst_decodes_the_same_as_its_rows_do_one_at_a_time() {
    // A MIDI port hands over whatever it has, and the desk is played with two
    // hands. If the table only ever holds one message the decoder's state
    // machine is never asked to carry on.
    let mut stream = Vec::new();
    let mut expected = Vec::new();
    for row in INBOUND {
        stream.extend_from_slice(row.bytes);
        expected.push(row.event);
    }
    assert_eq!(decode_events(&stream), expected);
}

#[test]
fn the_same_burst_with_the_status_bytes_left_out_decodes_identically() {
    // IMPLEMENTATION_PLAN.md S19: "Running status decoded identically to
    // explicit status" - on the real table rather than on a contrived pair of
    // notes, and over a stream that changes status many times so that dropping
    // the *first* of a run is not enough to pass.
    let mut explicit = Vec::new();
    let mut running = Vec::new();
    let mut previous_status = None;
    let mut expected = Vec::new();
    for row in INBOUND {
        explicit.extend_from_slice(row.bytes);
        expected.push(row.event);
        let (status, data) = row.bytes.split_first().expect("a status byte");
        if previous_status == Some(*status) {
            running.extend_from_slice(data);
        } else {
            running.extend_from_slice(row.bytes);
            previous_status = Some(*status);
        }
    }
    assert!(
        running.len() < explicit.len(),
        "no status byte was actually omitted"
    );
    assert_eq!(decode_events(&running), decode_events(&explicit));
    assert_eq!(decode_events(&running), expected);
}

#[test]
fn a_scribble_strip_message_split_across_packets_arrives_once_and_whole() {
    // docs/MCU_MAPPING.md §6, and the reason the SysEx buffer exists: USB MIDI
    // hands over four bytes at a time, so the longest message the protocol has
    // never arrives in one piece.
    let text = b"SPOT 1 SPOT 2 SPOT 3 SPOT 4 SPOT 5 SPOT 6 SPOT 7 SPOT 8";
    let whole = encode_feedback(&Feedback::DisplayText {
        offset: 0,
        text: text.as_slice(),
    });
    assert!(whole.len() > 60, "the message is worth splitting");

    let mut decoder = MidiDecoder::new();
    let mut seen = Vec::new();
    for (index, packet) in whole.chunks(4).enumerate() {
        decoder.push(packet, Duration::from_millis(index as u64), |message| {
            if let Some(Feedback::DisplayText { offset, text }) =
                Feedback::from_midi(&X_TOUCH, &message)
            {
                seen.push((offset, text.to_vec()));
            }
        });
    }
    assert_eq!(seen, vec![(0u8, text.to_vec())]);
    assert_eq!(decoder.counters().messages, 1);
    assert_eq!(decoder.counters().discarded(), 0);
}

// --- Properties ------------------------------------------------------------
//
// A table cannot enumerate 16 384 fader positions on nine faders, or 127 step
// counts on eight V-Pots. These say the same thing about the whole space.

fn any_strip() -> impl Strategy<Value = u8> {
    0..X_TOUCH.strips
}

fn any_fader() -> impl Strategy<Value = Fader> {
    prop_oneof![any_strip().prop_map(Fader::Strip), Just(Fader::Main)]
}

fn any_button() -> impl Strategy<Value = ButtonId> {
    prop_oneof![
        (any_strip(), 0..StripButton::ALL.len()).prop_map(|(strip, index)| ButtonId::Strip {
            strip,
            button: StripButton::ALL[index],
        }),
        (0..GlobalButton::ALL.len()).prop_map(|index| ButtonId::Global(GlobalButton::ALL[index])),
    ]
}

fn any_event() -> impl Strategy<Value = ControlEvent> {
    prop_oneof![
        (any_button(), any::<bool>())
            .prop_map(|(button, pressed)| ControlEvent::Button { button, pressed }),
        (any_fader(), any::<bool>())
            .prop_map(|(fader, touched)| ControlEvent::Touch { fader, touched }),
        (any_fader(), 0..=FADER_MAX)
            .prop_map(|(fader, position)| ControlEvent::Move { fader, position }),
        (any_strip(), -63i8..=63).prop_map(|(strip, steps)| ControlEvent::VPot { strip, steps }),
        (-63i8..=63).prop_map(|steps| ControlEvent::Jog { steps }),
    ]
}

fn any_ring() -> impl Strategy<Value = VPotRing> {
    (0..RingMode::ALL.len(), any::<bool>()).prop_flat_map(|(index, led)| {
        let mode = RingMode::ALL[index];
        (0..=mode.max_position()).prop_map(move |position| VPotRing {
            mode,
            position,
            led,
        })
    })
}

fn any_feedback() -> impl Strategy<Value = Feedback<'static>> {
    prop_oneof![
        (any_button(), 0..3usize).prop_map(|(button, index)| Feedback::Led {
            button,
            state: [LedState::Off, LedState::Flashing, LedState::On][index],
        }),
        (any_fader(), 0..=FADER_MAX)
            .prop_map(|(fader, position)| Feedback::Move { fader, position }),
        (any_strip(), any_ring()).prop_map(|(strip, ring)| Feedback::Ring { strip, ring }),
        (any_strip(), 0..=13u8).prop_map(|(strip, level)| Feedback::Meter {
            strip,
            signal: MeterSignal::Level(level)
        }),
        (any_strip(), any::<bool>()).prop_map(|(strip, over)| Feedback::Meter {
            strip,
            signal: MeterSignal::Overload(over)
        }),
        (0..X_TOUCH.segments, 0..=0x3Fu8, any::<bool>()).prop_map(|(digit, code, dot)| {
            Feedback::Segment {
                digit,
                character: SegmentChar { code, dot },
            }
        }),
        prop::array::uniform8(0..StripColor::ALL.len()).prop_map(|indexes| {
            let mut colors = [StripColor::Off; SCRIBBLE_STRIP_COLORS];
            for (slot, index) in colors.iter_mut().zip(indexes) {
                *slot = StripColor::ALL[index];
            }
            Feedback::DisplayColors(colors)
        }),
        any::<bool>().prop_map(|vertical| Feedback::MeterMode(if vertical {
            LcdMeterMode::Vertical
        } else {
            LcdMeterMode::Horizontal
        })),
        Just(Feedback::DeviceQuery),
    ]
}

proptest! {
    #[test]
    fn every_valid_event_writes_bytes_that_read_back_as_itself(event in any_event()) {
        let bytes = encode_event(&event);
        prop_assert_eq!(decode_events(&bytes), vec![event]);
    }

    #[test]
    fn every_valid_event_writes_the_same_bytes_the_second_time(event in any_event()) {
        // The other direction of the round trip: bytes -> event -> bytes. It is
        // the half that catches a decoder which threw information away and an
        // encoder that guessed it back.
        let bytes = encode_event(&event);
        let decoded = decode_events(&bytes);
        let again = decoded.first().map(encode_event);
        prop_assert_eq!(again, Some(bytes));
    }

    #[test]
    fn every_valid_feedback_writes_bytes_that_read_back_as_itself(feedback in any_feedback()) {
        let bytes = encode_feedback(&feedback);
        let mut decoder = MidiDecoder::new();
        let mut seen = 0usize;
        decoder.push(&bytes, NOW, |message| {
            if let Some(read) = Feedback::from_midi(&X_TOUCH, &message) {
                seen += 1;
                assert_eq!(read, feedback);
                let mut buf = [0u8; MAX_MESSAGE_BYTES];
                let written = read.encode_into(&X_TOUCH, &mut buf).expect("it was just read");
                assert_eq!(&buf[..written], bytes.as_slice());
            }
        });
        prop_assert_eq!(seen, 1);
    }

    #[test]
    fn scribble_strip_text_of_any_length_the_buffer_holds_round_trips(
        offset in 0..X_TOUCH.lcd_buffer_len,
        text in prop::collection::vec(0x20..=0x7Fu8, 0..40),
    ) {
        // The offset and the length interact - a message that fits at 0 does
        // not fit at 100 - so the pair is generated rather than either alone.
        let text = &text[..text.len().min(usize::from(X_TOUCH.lcd_buffer_len - offset))];
        let feedback = Feedback::DisplayText { offset, text };
        let bytes = encode_feedback(&feedback);
        let mut decoder = MidiDecoder::new();
        let mut seen = 0usize;
        decoder.push(&bytes, NOW, |message| {
            if let Some(read) = Feedback::from_midi(&X_TOUCH, &message) {
                seen += 1;
                assert_eq!(read, feedback);
            }
        });
        prop_assert_eq!(seen, 1);
    }
}
