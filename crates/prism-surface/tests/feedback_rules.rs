//! The four rules of `docs/MCU_MAPPING.md` §5, as the exit criteria state them.
//!
//! | Criterion | Where |
//! |---|---|
//! | No outbound pitch bend while a fader is touched; **exactly one** resync 150 ms after release | [`nothing_reaches_a_fader_a_hand_is_on`], [`releasing_a_fader_resynchronises_it_exactly_once`] |
//! | 1000 value changes in 100 ms produce **at most 3** messages for that control | [`a_thousand_changes_in_a_tenth_of_a_second_cost_three_messages`] |
//! | Priority under bandwidth pressure: faders → LEDs → LCD → meters | [`under_pressure_the_order_is_faders_leds_displays_meters`], [`a_meter_is_dropped_before_anything_that_matters`] |
//! | The device disappearing leaves the engine alone | [`a_desk_that_vanishes_mid_show_costs_the_engine_nothing`] |
//!
//! # Everything here is asserted on the bytes
//!
//! A message is recorded as what it would put on the wire, and classified by a
//! `match` on the status byte written out of `docs/MCU_MAPPING.md` §2.2 by hand —
//! **not** by [`prism_surface::Priority::of`], which is the code under test. S19
//! and S20 both found the same trap from the other end: a round trip through a
//! function and its own inverse agrees with itself whatever it says, and a test
//! that classified messages with the classifier it is checking would pass with
//! the whole order reversed.

use std::time::Duration;

use prism_surface::{
    ButtonId, DisplayLine, Fader, Feedback, GlobalButton, LedState, MAX_MESSAGE_BYTES, MeterSignal,
    RingMode, StripButton, StripColor, SurfaceController, SurfaceHealth, SurfaceTiming, X_TOUCH,
};

/// One message, as bytes and as the instant it left.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Wire {
    at: Duration,
    bytes: Vec<u8>,
}

/// The four classes of `docs/MCU_MAPPING.md` §5.2, read off the wire by hand.
///
/// Transcribed from §2.2 rather than taken from the crate: pitch bend is a
/// motor fader, a Note On is a button LED, the ring LEDs are CC 48–55, the
/// scribble strips are SysEx and the 7-segment display is CC 64–75, and channel
/// pressure is a meter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Class {
    Fader,
    Led,
    Display,
    Meter,
    Other,
}

fn class(bytes: &[u8]) -> Class {
    match (bytes.first().copied(), bytes.get(1).copied()) {
        (Some(0xE0..=0xEF), _) => Class::Fader,
        (Some(0x90), _) => Class::Led,
        (Some(0xB0), Some(48..=55)) => Class::Led,
        (Some(0xB0), Some(64..=75)) => Class::Display,
        (Some(0xF0), _) if bytes.get(5) == Some(&0x12) || bytes.get(5) == Some(&0x72) => {
            Class::Display
        }
        (Some(0xD0), _) => Class::Meter,
        _ => Class::Other,
    }
}

/// Pumps once per millisecond from `from` up to and including `until`.
fn drain(controller: &mut SurfaceController, from: Duration, until: Duration) -> Vec<Wire> {
    let mut out = Vec::new();
    let mut now = from;
    while now <= until {
        collect(controller, now, &mut out);
        now = now.saturating_add(Duration::from_millis(1));
    }
    out
}

/// One pump, recording what went out.
fn collect(controller: &mut SurfaceController, now: Duration, out: &mut Vec<Wire>) {
    controller.pump(now, |feedback| {
        let mut buf = [0u8; MAX_MESSAGE_BYTES];
        let written = feedback
            .encode_into(&X_TOUCH, &mut buf)
            .expect("layer 2 only ever produces messages layer 1 can write");
        out.push(Wire {
            at: now,
            bytes: buf.get(..written).expect("written fits").to_vec(),
        });
    });
}

/// A controller with a surface attached, its resync burst finished, and one
/// second on the clock.
fn settled() -> (SurfaceController, Duration) {
    let mut controller = SurfaceController::new(X_TOUCH);
    controller.connected(Duration::ZERO);
    let start = Duration::from_secs(1);
    assert_eq!(
        drain(&mut controller, Duration::ZERO, start).len(),
        156,
        "the whole surface should have been drawn once"
    );
    (controller, start)
}

/// Pitch-bend messages addressed to one fader's channel.
fn moves(sent: &[Wire], channel: u8) -> Vec<&Wire> {
    sent.iter()
        .filter(|message| message.bytes.first() == Some(&(0xE0 | channel)))
        .collect()
}

// --------------------------------------------------------------- touch suppression

#[test]
fn nothing_reaches_a_fader_a_hand_is_on() {
    // §5.1, and it is not an optimisation: the engine echoes the value back, the
    // motor drives to it, the movement reports a new value, and the loop
    // oscillates under the operator's finger.
    let (mut controller, start) = settled();
    controller.push(&[0x90, 104, 0x7F], start, |_| {});
    assert!(controller.is_touched(Fader::Strip(0)));

    let mut sent = Vec::new();
    let mut now = start;
    // Half a second of the show moving that fader as hard as it can.
    for step in 0..500u16 {
        controller.set_fader(Fader::Strip(0), step.wrapping_mul(97));
        collect(&mut controller, now, &mut sent);
        now = now.saturating_add(Duration::from_millis(1));
    }
    assert!(
        moves(&sent, 0).is_empty(),
        "the motor was driven while a hand was on it"
    );
    assert!(controller.counters().touch_suppressed >= 499);
}

#[test]
fn a_touched_fader_suppresses_nothing_but_itself() {
    // The other half, and the one a blunt implementation gets wrong: a hand on
    // strip 1 must not freeze the other eight faders, which are the executors
    // the operator is watching.
    let (mut controller, start) = settled();
    controller.push(&[0x90, 104, 0x7F], start, |_| {});
    controller.set_fader(Fader::Strip(0), 30_000);
    controller.set_fader(Fader::Strip(1), 30_000);
    controller.set_fader(Fader::Main, 30_000);
    let sent = drain(&mut controller, start, start + Duration::from_millis(200));
    assert!(moves(&sent, 0).is_empty());
    assert_eq!(moves(&sent, 1).len(), 1);
    assert_eq!(moves(&sent, 8).len(), 1);
}

#[test]
fn releasing_a_fader_resynchronises_it_exactly_once() {
    // §5.1: on release it waits 150 ms, then resynchronises to the authoritative
    // value. **Exactly one** message, and that is a guarantee rather than an
    // observation because a touch *invalidates* what this layer believes about
    // the fader — so the resync happens whether or not the value moved, and the
    // shadow model believes it afterwards, so nothing follows it.
    let (mut controller, start) = settled();
    controller.push(&[0x90, 104, 0x7F], start, |_| {});
    controller.set_fader(Fader::Strip(0), 0x8000);
    let touching = drain(&mut controller, start, start + Duration::from_millis(300));
    assert!(moves(&touching, 0).is_empty());

    let released = start + Duration::from_millis(300);
    controller.push(&[0x90, 104, 0x00], released, |_| {});
    assert!(!controller.is_touched(Fader::Strip(0)));

    // Nothing in the 150 ms the document asks for.
    let waiting = drain(
        &mut controller,
        released,
        released + Duration::from_millis(149),
    );
    assert!(
        moves(&waiting, 0).is_empty(),
        "the fader was resynchronised before the delay was up"
    );

    // Then exactly one, inside a frame of the deadline.
    let after = drain(
        &mut controller,
        released + Duration::from_millis(150),
        released + Duration::from_millis(500),
    );
    let resync = moves(&after, 0);
    assert_eq!(resync.len(), 1, "expected exactly one resynchronisation");
    // Worked out by hand off the wire: level 0x8000 of 0xFFFF over a top of
    // travel of 16380 is 8190, and 8190 is 63 x 128 + 126 — LSB first.
    assert_eq!(
        resync.first().map(|message| message.bytes.as_slice()),
        Some(&[0xE0, 126, 63][..])
    );
    assert!(
        resync
            .first()
            .is_some_and(|message| message.at >= released + Duration::from_millis(150)),
        "the one message arrived before the deadline"
    );
    assert_eq!(controller.counters().resyncs, 1);

    // And nothing else, ever, until something actually changes.
    let quiet = drain(
        &mut controller,
        released + Duration::from_millis(500),
        released + Duration::from_secs(3),
    );
    assert!(moves(&quiet, 0).is_empty());
    assert_eq!(controller.counters().resyncs, 1);
}

#[test]
fn a_fader_nobody_moved_is_still_resynchronised_after_a_release() {
    // The case that separates *invalidate on touch* from *send if the value
    // changed*: the operator moved the motor, so the desk is somewhere this
    // layer never put it, and there is nothing in the picture to notice.
    let (mut controller, start) = settled();
    controller.push(&[0x90, 108, 0x7F], start, |_| {});
    let released = start + Duration::from_millis(100);
    controller.push(&[0x90, 108, 0x00], released, |_| {});
    let sent = drain(
        &mut controller,
        start,
        released + Duration::from_millis(400),
    );
    let resync = moves(&sent, 4);
    assert_eq!(resync.len(), 1);
    assert_eq!(
        resync.first().map(|message| message.bytes.as_slice()),
        Some(&[0xE4, 0x00, 0x00][..]),
        "the fader is at zero in the picture and that is what has to be restated"
    );
}

// -------------------------------------------------------------------- coalescing

#[test]
fn a_thousand_changes_in_a_tenth_of_a_second_cost_three_messages() {
    // The exit criterion, and §5.2's reason for existing: USB MIDI cannot absorb
    // an unthrottled delta stream, and S20 found that trying is worse than
    // laggy — it can stop the surface transmitting until it is power-cycled.
    let (mut controller, start) = settled();
    let mut sent = Vec::new();
    for step in 0..1000u32 {
        let now = start + Duration::from_micros(u64::from(step) * 100);
        controller.set_fader(Fader::Strip(2), (step * 65) as u16);
        collect(&mut controller, now, &mut sent);
    }
    let messages = moves(&sent, 2);
    assert!(
        messages.len() <= 3,
        "{} messages for one control in 100 ms",
        messages.len()
    );
    assert!(!messages.is_empty(), "the desk has to see something");

    // ...and the value that survives is the last one, not whichever frame
    // boundary happened to catch a stale reading.
    let last = 999u32 * 65;
    controller.set_fader(Fader::Strip(2), last as u16);
    let settle = start + Duration::from_millis(200);
    drain(&mut controller, settle, settle + Duration::from_millis(100));
    assert_eq!(
        controller.shadow().fader(Fader::Strip(2)),
        Some(last as u16)
    );
}

#[test]
fn coalescing_holds_for_every_kind_of_control_at_once() {
    // A desk under a running show changes everything at once. The bound is per
    // control, so a thousand changes to eight strips is still three messages
    // each, and the total is what the port has to carry.
    let (mut controller, start) = settled();
    let mut sent = Vec::new();
    for step in 0..1000u32 {
        let now = start + Duration::from_micros(u64::from(step) * 100);
        for strip in 0..8u8 {
            controller.set_fader(Fader::Strip(strip), (step * 61) as u16);
            controller.set_meter(strip, MeterSignal::Level((step % 13) as u8));
            controller.set_ring(strip, RingMode::Wrap, (step % 12) as u8);
        }
        collect(&mut controller, now, &mut sent);
    }
    for strip in 0..8u8 {
        assert!(moves(&sent, strip).len() <= 3, "strip {strip}");
    }
    let meters = sent
        .iter()
        .filter(|message| class(&message.bytes) == Class::Meter)
        .count();
    assert!(meters <= 3 * 8, "{meters} meter messages in 100 ms");
}

// ---------------------------------------------------------------------- priority

#[test]
fn under_pressure_the_order_is_faders_leds_displays_meters() {
    // §5.2's order, and the pressure is real rather than simulated: everything
    // on the surface is different at once — 156 messages — and the send queue
    // lets one out per minimum gap.
    let (mut controller, start) = settled();
    dirty_everything(&mut controller);
    let sent = drain(&mut controller, start, start + Duration::from_secs(1));
    assert_eq!(sent.len(), 156);

    let classes: Vec<Class> = sent.iter().map(|message| class(&message.bytes)).collect();
    assert!(!classes.contains(&Class::Other), "an unclassified message");
    let mut sorted = classes.clone();
    sorted.sort_unstable();
    assert_eq!(classes, sorted, "the send order left the documented one");

    // The first messages out are the faders, all nine of them, before a single
    // LED — which is the claim "most visible, most misleading if stale" makes.
    assert!(
        classes.iter().take(9).all(|kind| *kind == Class::Fader),
        "something went out in front of the motor faders"
    );
    // And the meters are the tail.
    assert!(
        classes
            .iter()
            .rev()
            .take(8)
            .all(|kind| *kind == Class::Meter),
        "something went out behind the meters"
    );
}

#[test]
fn a_narrow_window_carries_the_faders_and_nothing_below_them() {
    // The same order under a budget rather than over a second: the first six
    // messages the port is given, out of the hundred and fifty-six that are
    // waiting. §5.2 says which six they have to be.
    let (mut controller, start) = settled();
    dirty_everything(&mut controller);
    let first = take(&mut controller, start, 6);
    assert_eq!(first.len(), 6, "nothing went out at all");
    assert!(
        first
            .iter()
            .all(|message| class(&message.bytes) == Class::Fader),
        "a lower class overtook the motor faders"
    );
    assert!(controller.pending() > 0, "the rest should still be waiting");
}

/// Pumps a millisecond at a time until `count` messages have gone out, and
/// stops there — a caller with only that much bandwidth to give.
fn take(controller: &mut SurfaceController, from: Duration, count: usize) -> Vec<Wire> {
    let mut out = Vec::new();
    let mut now = from;
    let deadline = from + Duration::from_secs(5);
    while out.len() < count && now <= deadline {
        collect(controller, now, &mut out);
        now = now.saturating_add(Duration::from_millis(1));
    }
    out
}

#[test]
fn a_meter_is_dropped_before_anything_that_matters() {
    // §5.2: meters are decorative and dropped first. The test is a caller too
    // slow to carry everything — one message per frame — with meters changing
    // every frame the way a real level does. The LED gets through; the meters
    // starve, and that is correct, because a dropped meter falls rather than
    // freezing (§2.7).
    let (mut controller, start) = settled();
    let timing = controller.timing();
    controller.set_led(ButtonId::Global(GlobalButton::Play), LedState::On);
    let mut sent = Vec::new();
    let mut now = start;
    for step in 0..20u8 {
        for strip in 0..8u8 {
            controller.set_meter(strip, MeterSignal::Level(step % 13));
        }
        collect(&mut controller, now, &mut sent);
        now = now.saturating_add(timing.frame);
    }
    let first_led = sent
        .iter()
        .position(|message| class(&message.bytes) == Class::Led);
    let first_meter = sent
        .iter()
        .position(|message| class(&message.bytes) == Class::Meter);
    assert_eq!(first_led, Some(0), "the LED did not go first");
    assert!(
        first_meter.is_none_or(|meter| meter > 0),
        "a meter overtook the LED"
    );
    // The meters do get through once nothing better is waiting.
    assert!(first_meter.is_some(), "the meters never went at all");
}

/// Changes every control on the surface, so the next frame's diff is the whole
/// of it.
fn dirty_everything(controller: &mut SurfaceController) {
    for strip in 0..8u8 {
        controller.set_fader(Fader::Strip(strip), 0x4000 + u16::from(strip));
        controller.set_ring(strip, RingMode::Spread, 3);
        controller.set_meter(strip, MeterSignal::Level(7));
        controller.set_color(strip, StripColor::Magenta);
        for line in DisplayLine::ALL {
            controller.set_text(strip, line, "EXEC");
        }
        for button in StripButton::ALL {
            controller.set_led(ButtonId::Strip { strip, button }, LedState::On);
        }
    }
    controller.set_fader(Fader::Main, 0x2000);
    controller.set_segment_text("PAGE-1");
    for button in GlobalButton::ALL {
        controller.set_led(ButtonId::Global(button), LedState::Flashing);
    }
}

// ------------------------------------------------------------------- the device

#[test]
fn a_desk_that_vanishes_mid_show_costs_the_engine_nothing() {
    // §5.3 and the exit criterion. The engine is represented here by what it
    // does: it goes on setting state and it goes on receiving events. Nothing
    // about the desk's absence reaches it — no error to handle, no call that
    // fails, no state rolled back — and what happened while it was away is on
    // the surface when it returns.
    let (mut controller, start) = settled();
    controller.push(&[0x90, 104, 0x7F], start, |_| {});

    controller.disconnected();
    assert_eq!(controller.health(), SurfaceHealth::Disconnected);
    // A hand that was on a fader when the cable went is not still on it when the
    // desk comes back, or that fader would be suppressed for ever.
    assert!(!controller.is_touched(Fader::Strip(0)));

    let dark = start + Duration::from_secs(1);
    for step in 0..200u16 {
        controller.set_fader(Fader::Strip(3), step * 300);
        controller.set_text(3, DisplayLine::Upper, "GONE");
        controller.set_led(ButtonId::Global(GlobalButton::Save), LedState::On);
    }
    assert!(
        drain(&mut controller, dark, dark + Duration::from_secs(2)).is_empty(),
        "something was written to a port that is not there"
    );

    let back = dark + Duration::from_secs(2);
    controller.connected(back);
    let sent = drain(&mut controller, back, back + Duration::from_secs(1));
    assert_eq!(sent.len(), 156, "the desk was not redrawn from scratch");
    assert_eq!(controller.desired().fader(Fader::Strip(3)), Some(199 * 300));
    assert_eq!(controller.desired(), controller.shadow());
    assert_eq!(
        controller.desired().text(3, DisplayLine::Upper),
        Some(b"GONE   ")
    );

    // The surface works afterwards, which is the part a resync burst is for.
    let mut events = Vec::new();
    controller.push(&[0x90, 94, 0x7F], back + Duration::from_secs(1), |event| {
        events.push(event);
    });
    assert_eq!(events.len(), 1);
    assert_eq!(controller.health(), SurfaceHealth::Live);
}

#[test]
fn a_desk_that_goes_quiet_is_named_a_desk_and_not_a_cable() {
    // The fault §5.3 did not cover until S20 found it: the port stays open,
    // writes still land on the display, and the surface's transmitter is dead.
    // The whole value of noticing is in the words, so the words are asserted.
    let timing = SurfaceTiming {
        silence: Duration::from_millis(200),
        probe: Duration::from_millis(50),
        ..SurfaceTiming::DEFAULT
    };
    let mut controller = SurfaceController::with_timing(X_TOUCH, timing);
    controller.connected(Duration::ZERO);
    drain(&mut controller, Duration::ZERO, Duration::from_millis(500));
    controller.push(&[0x90, 91, 0x7F], Duration::from_millis(500), |_| {});
    assert_eq!(controller.health(), SurfaceHealth::Live);
    assert_eq!(controller.health().remedy(), None);

    let quiet = drain(
        &mut controller,
        Duration::from_millis(500),
        Duration::from_secs(2),
    );
    // One question, and it is the device query — the only message this surface
    // ever answers, sent once and never as a keep-alive (§2.7).
    assert_eq!(quiet.len(), 1);
    assert_eq!(
        quiet.first().map(|message| message.bytes.as_slice()),
        Some(&[0xF0, 0x00, 0x00, 0x66, 0x14, 0x00, 0xF7][..])
    );
    assert_eq!(controller.health(), SurfaceHealth::Unresponsive);
    let remedy = controller.health().remedy().unwrap_or_default();
    assert!(
        remedy.contains("power-cycle"),
        "the remedy has to say power-cycle: {remedy}"
    );
    assert!(
        remedy.contains("will not"),
        "and it has to say that reconnecting is not it: {remedy}"
    );

    // Writes still land — the desk is deaf-mute in one direction only, so the
    // picture goes on being maintained on a display that still works.
    controller.set_text(0, DisplayLine::Upper, "DEAD");
    let writing = drain(
        &mut controller,
        Duration::from_secs(2),
        Duration::from_secs(3),
    );
    assert_eq!(writing.len(), 1);
    assert_eq!(class(&writing[0].bytes), Class::Display);
}

#[test]
fn the_feedback_a_test_reads_is_the_feedback_the_desk_would_get() {
    // A guard on this file rather than on the crate: every assertion above reads
    // bytes, so a message that encoded to something else entirely would slip
    // through as long as it was consistent. Decoding one back through layer 1
    // closes that — and layer 1's decoder is written against the document, not
    // against the shadow model.
    let (mut controller, start) = settled();
    controller.set_led(ButtonId::Global(GlobalButton::Save), LedState::On);
    let sent = drain(&mut controller, start, start + Duration::from_millis(100));
    assert_eq!(sent.len(), 1);
    let bytes = sent
        .first()
        .map(|message| message.bytes.clone())
        .unwrap_or_default();
    assert_eq!(bytes, vec![0x90, 80, 0x7F]);
    let decoded = Feedback::from_midi(
        &X_TOUCH,
        &prism_surface::MidiMessage::NoteOn {
            channel: 0,
            note: 80,
            velocity: 0x7F,
        },
    );
    assert_eq!(
        decoded,
        Some(Feedback::Led {
            button: ButtonId::Global(GlobalButton::Save),
            state: LedState::On
        })
    );
}
