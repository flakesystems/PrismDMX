//! The desk's own bytes, replayed through the codec — and **no hardware
//! required**.
//!
//! S20 held the tables of `docs/MCU_MAPPING.md` §2 against a real Behringer
//! X-Touch (MC mode, USB, firmware V1.25, serial `0156406`). The captures in
//! `tests/captures/` are what that surface sent while every control on it was
//! worked in turn, recorded by `tools/xtouch-probe`. This target replays them.
//!
//! It exists because a verification session is otherwise a paragraph in a
//! document. `verified: true` in [`prism_surface::X_TOUCH`] is a claim about an
//! evening in August 2026; these files are the evidence, and they turn the claim
//! into a test that runs on every commit, on a build server, for ever, with
//! nothing plugged in. If a later session edits a note number, the desk's own
//! recording is what says no.
//!
//! The strongest assertion here is the last one, and it is the one S19's
//! round-trip table could not make: **every message the surface sent
//! re-encodes, byte for byte, to exactly the bytes it sent.** The hand-written
//! table in `round_trip.rs` says the codec agrees with the *document*; this says
//! it agrees with the *device*.
//!
//! | Capture | What was done to the desk |
//! |---|---|
//! | `strip-buttons.txt` | every button on all eight channel strips, strip by strip |
//! | `global-buttons.txt` | the 64-button panel, each one lit by the host and then pressed |
//! | `faders.txt` | all nine faders touched and swept end to end |
//! | `encoders.txt` | all eight V-Pots and the jog wheel, slowly and at speed |

use std::collections::BTreeSet;
use std::time::Duration;

use prism_surface::{
    ButtonId, ControlEvent, FADER_MAX, Fader, GlobalButton, MAX_MESSAGE_BYTES, McuCodec,
    StripButton, X_TOUCH,
};

/// One message as the surface sent it, with the events the codec made of it.
struct Decoded {
    bytes: Vec<u8>,
    events: Vec<ControlEvent>,
}

/// Replays a capture file, returning every message and what it decoded to.
///
/// Asserts as it goes that the codec threw nothing away: `wire.discarded()`
/// counts a malformed packet and `unmapped` counts a well-formed message this
/// profile does not describe. Both being zero over a whole capture is the claim
/// that **the profile describes everything this surface actually sends** — which
/// is the half of the verification a note-by-note table cannot state.
fn replay(capture: &str) -> Vec<Decoded> {
    let mut codec = McuCodec::new(X_TOUCH);
    let mut out = Vec::new();
    for (number, line) in capture.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (micros, rest) = line
            .split_once(' ')
            .unwrap_or_else(|| panic!("line {} has no timestamp: {line}", number + 1));
        let at = Duration::from_micros(
            micros
                .parse()
                .unwrap_or_else(|_| panic!("line {} has no timestamp: {line}", number + 1)),
        );
        let bytes: Vec<u8> = rest
            .split_whitespace()
            .map(|byte| {
                u8::from_str_radix(byte, 16)
                    .unwrap_or_else(|_| panic!("line {} has a bad byte: {byte}", number + 1))
            })
            .collect();
        let mut events = Vec::new();
        codec.push(&bytes, at, |event| events.push(event));
        out.push(Decoded { bytes, events });
    }

    let counters = codec.counters();
    assert_eq!(
        counters.wire.discarded(),
        0,
        "the surface sent something malformed: {counters:?}"
    );
    assert_eq!(
        counters.unmapped, 0,
        "the surface sent a well-formed message the profile does not describe: {counters:?}"
    );
    assert!(!out.is_empty(), "an empty capture proves nothing");
    out
}

fn strip_buttons() -> Vec<Decoded> {
    replay(include_str!("captures/strip-buttons.txt"))
}

fn global_buttons() -> Vec<Decoded> {
    replay(include_str!("captures/global-buttons.txt"))
}

fn faders() -> Vec<Decoded> {
    replay(include_str!("captures/faders.txt"))
}

fn encoders() -> Vec<Decoded> {
    replay(include_str!("captures/encoders.txt"))
}

/// Real messages from the captures, with the event each one means **worked out
/// by hand** rather than by the codec.
///
/// This table is why the byte round trip below is not the whole test, and a
/// mutation check is why the table exists: swapping the two halves of the 14-bit
/// fader split *in both directions* leaves
/// [`every_message_the_desk_sent_encodes_back_to_exactly_those_bytes`] green,
/// because a decoder composed with its own inverse still reproduces the input.
/// S19 found exactly that with a computed table and `docs/MCU_MAPPING.md` §2.6
/// records it; a capture does not escape it just by being real.
///
/// So each row below states the position or step count in decimal, computed off
/// the wire by a person: `E0 7C 7F` is 124 + 128 × 127 = 16380, and if the codec
/// ever answers 16256 + 127 for it, that is the fault this row exists to catch.
/// Every byte sequence is asserted to be present in the named capture as well,
/// so a row cannot survive as a claim about a message the desk never sent.
const HAND_READ: [(&str, &str, ControlEvent); 11] = [
    // Faders: the top of travel, a low position, and the main fader at rest.
    (
        "faders",
        "E0 7C 7F",
        ControlEvent::Move {
            fader: Fader::Strip(0),
            position: 16380,
        },
    ),
    (
        "faders",
        "E0 78 01",
        ControlEvent::Move {
            fader: Fader::Strip(0),
            position: 248,
        },
    ),
    (
        "faders",
        "E8 00 00",
        ControlEvent::Move {
            fader: Fader::Main,
            position: 0,
        },
    ),
    // Touch sensors: the first strip and the main fader, 104 and 112.
    (
        "faders",
        "90 68 7F",
        ControlEvent::Touch {
            fader: Fader::Strip(0),
            touched: true,
        },
    ),
    (
        "faders",
        "90 70 7F",
        ControlEvent::Touch {
            fader: Fader::Main,
            touched: true,
        },
    ),
    // Strip buttons: note 24 is strip 1's Select, note 39 is strip 8's V-Pot
    // push - the two ends of the block that tiles 0..39.
    (
        "strip-buttons",
        "90 18 7F",
        ControlEvent::Button {
            button: ButtonId::Strip {
                strip: 0,
                button: StripButton::Select,
            },
            pressed: true,
        },
    ),
    (
        "strip-buttons",
        "90 27 7F",
        ControlEvent::Button {
            button: ButtonId::Strip {
                strip: 7,
                button: StripButton::VPotPush,
            },
            pressed: true,
        },
    ),
    // A release, as the surface writes one: a Note On with velocity 0.
    (
        "global-buttons",
        "90 2A 00",
        ControlEvent::Button {
            button: ButtonId::Global(GlobalButton::AssignPan),
            pressed: false,
        },
    ),
    // The relative controls, both carrying the sign bit. Read as two's
    // complement, 0x47 would be +71 and 0x48 would be +72.
    (
        "encoders",
        "B0 10 47",
        ControlEvent::VPot {
            strip: 0,
            steps: -7,
        },
    ),
    (
        "encoders",
        "B0 17 48",
        ControlEvent::VPot {
            strip: 7,
            steps: -8,
        },
    ),
    ("encoders", "B0 3C 01", ControlEvent::Jog { steps: 1 }),
];

#[test]
fn messages_read_off_the_wire_by_hand_decode_to_what_they_say() {
    for (capture, hex, expected) in HAND_READ {
        let text = match capture {
            "faders" => include_str!("captures/faders.txt"),
            "encoders" => include_str!("captures/encoders.txt"),
            "strip-buttons" => include_str!("captures/strip-buttons.txt"),
            "global-buttons" => include_str!("captures/global-buttons.txt"),
            other => panic!("no capture called {other}"),
        };
        assert!(
            text.lines().any(|line| line.trim_end().ends_with(hex)),
            "{hex} is not in {capture}.txt, so the row is a claim about nothing"
        );
        let bytes: Vec<u8> = hex
            .split_whitespace()
            .map(|byte| u8::from_str_radix(byte, 16).expect("a hex byte"))
            .collect();
        let mut codec = McuCodec::new(X_TOUCH);
        let mut events = Vec::new();
        codec.push(&bytes, Duration::ZERO, |event| events.push(event));
        assert_eq!(events, vec![expected], "{hex} from {capture}.txt");
    }
}

/// Every message from all four captures, re-encoded and compared with the bytes
/// the surface sent.
///
/// This is the round-trip criterion of S19 with the document taken out of it. A
/// swapped 14-bit fader split, a note table shifted by one, a release written as
/// a real Note Off: each of them turns this red, and none of them can be argued
/// with, because the right-hand side of the comparison was produced by the
/// device.
#[test]
fn every_message_the_desk_sent_encodes_back_to_exactly_those_bytes() {
    let mut checked = 0_usize;
    for capture in [strip_buttons(), global_buttons(), faders(), encoders()] {
        for message in capture {
            assert_eq!(
                message.events.len(),
                1,
                "{:02X?} decoded to {} events",
                message.bytes,
                message.events.len()
            );
            let mut buf = [0u8; MAX_MESSAGE_BYTES];
            let written = message.events[0]
                .encode_into(&X_TOUCH, &mut buf)
                .unwrap_or_else(|error| {
                    panic!("{:02X?} would not re-encode: {error}", message.bytes)
                });
            assert_eq!(
                &buf[..written],
                &message.bytes[..],
                "{:?} came from {:02X?} and encodes to {:02X?}",
                message.events[0],
                message.bytes,
                &buf[..written]
            );
            checked += 1;
        }
    }
    // Four captures, a few thousand messages. Stated as a floor so that a
    // capture file emptied by accident cannot make this test pass vacuously.
    assert!(checked > 2000, "only {checked} messages were checked");
}

#[test]
fn all_forty_strip_buttons_reported_the_note_the_table_gives_them() {
    // The walk was done strip by strip rather than row by row, which is why this
    // asserts the *set* and not an order. Every one of the forty combinations
    // has to appear, and each has to have arrived on the note the profile
    // computes for it - `replay` has already refused anything the profile could
    // not place at all.
    let mut seen = BTreeSet::new();
    for message in strip_buttons() {
        if let ControlEvent::Button {
            button: ButtonId::Strip { strip, button },
            pressed: true,
        } = message.events[0]
        {
            assert_eq!(
                X_TOUCH.strip_note(strip, button),
                message.bytes.get(1).copied(),
                "strip {strip} {button} arrived on the wrong note"
            );
            seen.insert((strip, button));
        }
    }
    for strip in 0..X_TOUCH.strips {
        for button in StripButton::ALL {
            assert!(
                seen.contains(&(strip, button)),
                "strip {strip} {button} is not in the capture"
            );
        }
    }
    assert_eq!(seen.len(), 40);
}

#[test]
fn the_panel_buttons_answered_on_the_notes_their_leds_were_lit_on() {
    // `tools/xtouch-probe pair` lit one LED at a time and waited for a press, so
    // this capture is the *outbound* note map checked as well as the inbound
    // one: the button that lit is the button that was pressed. Sixty of the
    // sixty-four are here. Name/Value and SMPTE/Beats have no LED on this device
    // and were confirmed separately (notes 52 and 53, `docs/MCU_MAPPING.md`
    // §2.7); the two foot switch notes need a pedal nobody had.
    let mut seen = BTreeSet::new();
    for message in global_buttons() {
        if let ControlEvent::Button {
            button: ButtonId::Global(button),
            pressed: true,
        } = message.events[0]
        {
            assert_eq!(X_TOUCH.note_of(button), message.bytes.get(1).copied());
            seen.insert(button);
        }
    }
    assert_eq!(seen.len(), 60, "the pairing run answered sixty buttons");
}

#[test]
fn the_nine_faders_arrived_on_the_channels_and_touch_notes_the_table_gives_them() {
    let mut moved = BTreeSet::new();
    let mut touched = BTreeSet::new();
    for message in faders() {
        match message.events[0] {
            ControlEvent::Move { fader, .. } => {
                let status = message.bytes.first().copied().unwrap_or_default();
                assert_eq!(X_TOUCH.fader_channel(fader), Some(status & 0x0F));
                moved.insert(fader);
            }
            ControlEvent::Touch { fader, .. } => {
                assert_eq!(X_TOUCH.touch_note_of(fader), message.bytes.get(1).copied());
                touched.insert(fader);
            }
            other => panic!("a fader sweep produced {other:?}"),
        }
    }
    let expected: BTreeSet<Fader> = (0..X_TOUCH.strips)
        .map(Fader::Strip)
        .chain([Fader::Main])
        .collect();
    assert_eq!(moved, expected);
    assert_eq!(touched, expected);
}

/// The fader is 12-bit, and the top of its travel is 16380 rather than 16383.
///
/// Measured, and it is not a detail: a layer that scales an inbound position by
/// dividing by [`FADER_MAX`] gives an executor master 99.98 % with the fader
/// against its end stop, and a master that cannot reach full is a master that is
/// wrong. `McuProfile::fader_step` holds the granularity for that reason.
#[test]
fn the_fader_reports_in_steps_of_four_and_never_reaches_16383() {
    let mut highest = 0;
    let mut positions = 0_usize;
    for message in faders() {
        if let ControlEvent::Move { position, .. } = message.events[0] {
            assert_eq!(
                position % u16::from(X_TOUCH.fader_step),
                0,
                "{position} is not a multiple of the measured step"
            );
            highest = highest.max(position);
            positions += 1;
        }
    }
    assert!(positions > 500, "only {positions} fader positions");
    assert_eq!(highest, X_TOUCH.max_reported_position());
    assert_eq!(highest, 16380);
    assert!(highest < FADER_MAX);
}

/// The V-Pots accelerate and the jog wheel does not, on the same encoding.
///
/// `docs/MCU_MAPPING.md` §2.1 described both as "relative, sign-magnitude" and
/// left the magnitude open. They are not the same: a fast V-Pot turn carries up
/// to eight detents in one message, and the jog wheel sent nothing but ±1 in
/// four hundred messages however hard it was spun. A layer 2 that applied one
/// acceleration curve to both would be wrong about one of them.
#[test]
fn the_vpots_carry_a_magnitude_and_the_jog_wheel_only_ever_sends_one_step() {
    let mut vpot_magnitudes = BTreeSet::new();
    let mut jog_magnitudes = BTreeSet::new();
    let mut strips = BTreeSet::new();
    for message in encoders() {
        match message.events[0] {
            ControlEvent::VPot { strip, steps } => {
                assert_eq!(
                    X_TOUCH.vpot_controller(strip),
                    message.bytes.get(1).copied()
                );
                assert_ne!(steps, 0, "a zero-step message was never seen on the desk");
                vpot_magnitudes.insert(steps.abs());
                strips.insert(strip);
            }
            ControlEvent::Jog { steps } => {
                assert_eq!(message.bytes.get(1).copied(), Some(X_TOUCH.jog_cc));
                jog_magnitudes.insert(steps.abs());
            }
            other => panic!("an encoder turn produced {other:?}"),
        }
    }
    assert_eq!(strips.len(), usize::from(X_TOUCH.strips));
    assert_eq!(
        vpot_magnitudes,
        BTreeSet::from([1, 2, 3, 4, 5, 6, 7, 8]),
        "the measured V-Pot magnitudes are 1..8"
    );
    assert_eq!(jog_magnitudes, BTreeSet::from([1]));
}

/// Both directions of the relative encoding appear, which is what stops the sign
/// bit from being read as part of the magnitude.
#[test]
fn the_encoders_were_turned_both_ways() {
    let mut clockwise = 0_usize;
    let mut back = 0_usize;
    for message in encoders() {
        if let ControlEvent::VPot { steps, .. } | ControlEvent::Jog { steps } = message.events[0] {
            if steps > 0 {
                clockwise += 1;
            } else {
                back += 1;
            }
        }
    }
    assert!(clockwise > 100 && back > 100, "{clockwise} up, {back} down");
}

/// The profile that these captures verify is the one that says it is verified.
#[test]
fn the_profile_these_captures_belong_to_is_the_verified_one() {
    // A const block, as `profile.rs` does it: the flag is a constant, so this is
    // the build refusing rather than a test failing.
    const { assert!(X_TOUCH.verified) }
    assert_eq!(X_TOUCH.name, "Behringer X-Touch (MC mode)");
}
