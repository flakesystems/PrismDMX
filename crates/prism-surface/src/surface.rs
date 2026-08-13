//! Layer 2: the shadow model, the rules that keep a desk honest, and the send
//! queue that keeps it alive.
//!
//! [`SurfaceController`] is what the `midi-in` / `midi-out` threads of
//! `ARCHITECTURE_SPEC.md` §3 hold. Bytes go in and [`SurfaceEvent`]s come out;
//! show state goes in and [`Feedback`] messages come out, **paced**. It owns no
//! port, no thread and no clock — every entry point takes `now`, which is S19's
//! rule one layer up: the arrival time of a packet is a property of the packet,
//! and the send schedule is a property of the caller's loop.
//!
//! # The four rules, and why each of them is not an optimisation
//!
//! **Touch suppression** (`docs/MCU_MAPPING.md` §5.1). While a fader reports a
//! hand on it, nothing is sent to that fader. Without it the motor fights the
//! operator: the engine echoes the value back, the motor drives to it, the
//! movement reports a new value, and the loop oscillates. A touch also
//! *invalidates* what this layer believes about that fader — the operator has
//! moved the motor and the shadow is now a guess — which is what turns
//! §5.1's *resynchronise after 150 ms* into **exactly one** message rather than
//! into "one if the value happened to change".
//!
//! **Coalescing** (§5.2). The diff runs at most once per
//! [`SurfaceTiming::frame`], so a control that changed a thousand times between
//! two frames costs one message. Anything not sent this frame is simply still
//! different at the next one, because the shadow only moves when a message
//! actually goes out.
//!
//! **Priority** (§5.2). Faders, then LEDs, then the scribble strips and the
//! 7-segment display, then meters. Under pressure the tail of that order is
//! dropped, which is why meters are last: a dropped meter falls, it does not
//! freeze — the surface decays them in well under a second (§2.7).
//!
//! **Pacing** (§2.7, and this one was measured the hard way). Saturating the
//! X-Touch in both directions at once can stop it transmitting altogether while
//! it goes on receiving perfectly, and **only a power cycle brings it back**. So
//! [`SurfaceTiming::min_gap`] is a floor on the interval between two outbound
//! messages, enforced against the caller's clock rather than against a counter:
//! a controller pumped every millisecond sends at most one message per
//! millisecond, and one pumped once per frame sends one message per frame. The
//! 30 Hz of §5.2 is a safety limit, not a frame rate.
//!
//! # And the fault §5.3 does not cover
//!
//! That failure is **not** a disconnection: the port stays open and writes still
//! land on the display. A layer that only watched for disappearance would show a
//! green light beside a dead console. So a desk that *was* talking and has gone
//! quiet is asked once — the device query is the only reply this surface
//! generates, and asking once, with an empty send queue, is the opposite of the
//! condition that caused the fault. If that goes unanswered the health is
//! [`SurfaceHealth::Unresponsive`], whose remedy says **power-cycle**, because
//! reconnecting is precisely what does not work.

use std::time::Duration;

use prism_domain::RgbColor;

use crate::accel::{JOG_ACCELERATION, JogAcceleration, VPOT_ACCELERATION, VPotAcceleration};
use crate::codec::{CodecCounters, McuCodec};
use crate::color::quantize;
use crate::control::{ButtonId, ControlEvent};
use crate::feedback::{
    Feedback, LedState, METER_LEVEL_OVER, MeterSignal, RingMode, SegmentChar, StripColor, VPotRing,
};
use crate::model::{
    ALL_FADERS, Control, DISPLAY_LINES, DisplayLine, GLOBAL_BUTTONS, MAX_FADERS, MAX_SEGMENTS,
    MAX_STRIPS, STRIP_BUTTONS, STRIP_CHARS, SurfaceEvent, SurfaceMode, SurfaceState, fader_index,
};
use crate::profile::{Fader, GlobalButton, McuProfile, StripButton};

/// How many messages one frame's diff can hold.
///
/// Every control the shadow model knows about, at once, which is what a resync
/// burst is: nine faders, forty strip LEDs, sixty-two lit panel buttons, eight
/// rings, sixteen lines of text, one colour message, twelve digits and eight
/// meters — 156 on this surface. A test asserts the real figure rather than
/// trusting this comment.
const QUEUE_CAPACITY: usize = 176;

/// The clocks the outbound path runs on.
///
/// Every field is a floor or a delay rather than a rate, so a caller that pumps
/// slowly sends less and never more.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceTiming {
    /// Shortest interval between two diffs — `docs/MCU_MAPPING.md` §5.2's
    /// 30 Hz. A control that changes more often than this costs one message per
    /// frame and no more.
    pub frame: Duration,
    /// Shortest interval between two outbound messages.
    ///
    /// The two projects §2.3 cites found that a millisecond is enough to stop
    /// the surface losing the tail of a burst. S20 found something worse than a
    /// lost tail (§2.7), so this is a safety limit.
    pub min_gap: Duration,
    /// How long after a fader is released before it is resynchronised — §5.1's
    /// 150 ms.
    pub touch_resync: Duration,
    /// How long a desk that was talking may be silent before it is asked
    /// whether it is still there.
    pub silence: Duration,
    /// How long that one question may go unanswered before the desk is declared
    /// [`SurfaceHealth::Unresponsive`].
    pub probe: Duration,
}

impl SurfaceTiming {
    /// The timings `docs/MCU_MAPPING.md` §5 specifies, and two this crate
    /// chooses.
    ///
    /// The frame and the resynchronisation delay are the document's. The minimum
    /// gap is the community's measurement of the surface's pacing needs. The
    /// silence window and the probe timeout are neither: they are chosen so that
    /// a desk nobody is touching is not called dead — five seconds of no traffic
    /// during a show is ordinary, and the question that follows costs one
    /// seven-byte message.
    pub const DEFAULT: Self = Self {
        // 30 Hz. Written as nanoseconds because a third of a millisecond of
        // rounding, thirty times a second, is a frame every few minutes.
        frame: Duration::from_nanos(33_333_333),
        min_gap: Duration::from_millis(1),
        touch_resync: Duration::from_millis(150),
        silence: Duration::from_secs(5),
        probe: Duration::from_millis(500),
    };
}

impl Default for SurfaceTiming {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// What is known about the surface at the other end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SurfaceHealth {
    /// No surface. The engine does not care and neither does this type: the
    /// picture goes on being maintained and is sent when one arrives.
    #[default]
    Disconnected,
    /// A surface is there and has not said anything yet, which is ordinary — an
    /// X-Touch speaks only when it is touched.
    Connected,
    /// The surface has been heard from inside the silence window.
    Live,
    /// It has been quiet long enough that it has been asked, once, whether it is
    /// still there.
    Probing,
    /// It did not answer. **The port is still open and writes still land**; what
    /// has stopped is the surface's transmitter (§2.7).
    Unresponsive,
}

impl SurfaceHealth {
    /// What to tell the operator, if anything.
    ///
    /// The wording of the last one is the point of the whole state: the obvious
    /// advice — reconnect — is the one thing S20 established does **not** work.
    #[must_use]
    pub const fn remedy(self) -> Option<&'static str> {
        match self {
            Self::Disconnected => Some("no surface is connected"),
            Self::Unresponsive => Some(
                "the surface has stopped sending: power-cycle it. \
                 Reopening the port or restarting will not bring it back",
            ),
            Self::Connected | Self::Live | Self::Probing => None,
        }
    }

    /// Whether messages may be sent to the surface.
    #[must_use]
    pub const fn is_attached(self) -> bool {
        !matches!(self, Self::Disconnected)
    }
}

/// What layer 2 has done and not done.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SurfaceCounters {
    /// Messages handed to the sink.
    pub sent: u64,
    /// Changes overwritten by a later change before either was sent — what
    /// coalescing saved.
    pub superseded: u64,
    /// Changes to a fader that were not sent because a hand was on it.
    /// Overlaps [`superseded`](Self::superseded) deliberately: one counts the
    /// rule, the other the frame rate.
    pub touch_suppressed: u64,
    /// Faders resynchronised after a release.
    pub resyncs: u64,
    /// Inbound events dropped because the control is reserved — SMPTE/Beats,
    /// which switches the surface between hosts and is never PrismDMX's
    /// (`docs/MCU_MAPPING.md` §4.3).
    pub reserved: u64,
    /// Device queries sent. **At most one per silence**, and the reason it is
    /// counted is that the fault in §2.7 needed replies in flight: a number
    /// climbing here would mean somebody had started polling the handshake.
    pub probes: u64,
}

/// Which parts of the shadow model are believed rather than guessed.
///
/// A bit per control. Nothing is known before the first message goes out, which
/// is what makes a connect, a reconnect and a mode change produce a full resync
/// burst without any of them being a special case: they all clear this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Known {
    faders: u64,
    strip_leds: u64,
    global_leds: u64,
    rings: u64,
    text: u64,
    colors: bool,
    meters: u64,
    segments: u64,
}

impl Known {
    /// Nothing at all.
    const fn nothing() -> Self {
        Self {
            faders: 0,
            strip_leds: 0,
            global_leds: 0,
            rings: 0,
            text: 0,
            colors: false,
            meters: 0,
            segments: 0,
        }
    }
}

/// Whether bit `index` is set.
const fn bit(bits: u64, index: usize) -> bool {
    index < u64::BITS as usize && bits & (1u64 << index) != 0
}

/// `bits` with bit `index` set.
const fn with_bit(bits: u64, index: usize) -> u64 {
    if index < u64::BITS as usize {
        bits | (1u64 << index)
    } else {
        bits
    }
}

/// `bits` with bit `index` cleared.
const fn without_bit(bits: u64, index: usize) -> u64 {
    if index < u64::BITS as usize {
        bits & !(1u64 << index)
    } else {
        bits
    }
}

/// One thing this frame's diff found different, named by its control rather
/// than held as a message — a message would borrow the state it was built from
/// and would go stale the moment anything else changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pending {
    Fader(Fader),
    StripLed(u8, StripButton),
    GlobalLed(GlobalButton),
    Ring(u8),
    Text(u8, DisplayLine),
    Colors,
    Segment(u8),
    Meter(u8),
}

/// The surface as PrismDMX drives it: layer 2 of `docs/MCU_MAPPING.md` §1.
///
/// Holds the codec, the picture the show wants shown, the picture the desk was
/// last given, and the rules between them. Allocates nothing at any point, on
/// any path — see `tests/surface_allocations.rs`.
#[derive(Debug)]
pub struct SurfaceController {
    profile: McuProfile,
    mode: SurfaceMode,
    timing: SurfaceTiming,
    vpot: VPotAcceleration,
    jog: JogAcceleration,
    codec: McuCodec,
    desired: SurfaceState,
    shadow: SurfaceState,
    known: Known,
    touched: [bool; MAX_FADERS],
    resync_at: [Option<Duration>; MAX_FADERS],
    last_jog: Option<Duration>,
    next_frame_at: Duration,
    next_send_at: Duration,
    queue: [Pending; QUEUE_CAPACITY],
    queued: usize,
    cursor: usize,
    health: SurfaceHealth,
    last_inbound: Option<Duration>,
    probe_deadline: Option<Duration>,
    counters: SurfaceCounters,
}

impl SurfaceController {
    /// A controller for a surface, with the timings of
    /// [`SurfaceTiming::DEFAULT`].
    ///
    /// Starts [`SurfaceHealth::Disconnected`]: a controller exists before a port
    /// does, and [`connected`](Self::connected) is what starts the clocks and
    /// asks for the first resync burst.
    #[must_use]
    pub const fn new(profile: McuProfile) -> Self {
        Self::with_timing(profile, SurfaceTiming::DEFAULT)
    }

    /// A controller with timings of the caller's choosing.
    #[must_use]
    pub const fn with_timing(profile: McuProfile, timing: SurfaceTiming) -> Self {
        Self {
            profile,
            mode: SurfaceMode::Dedicated,
            timing,
            vpot: VPOT_ACCELERATION,
            jog: JOG_ACCELERATION,
            codec: McuCodec::new(profile),
            desired: SurfaceState::new(),
            shadow: SurfaceState::new(),
            known: Known::nothing(),
            touched: [false; MAX_FADERS],
            resync_at: [None; MAX_FADERS],
            last_jog: None,
            next_frame_at: Duration::ZERO,
            next_send_at: Duration::ZERO,
            queue: [Pending::Colors; QUEUE_CAPACITY],
            queued: 0,
            cursor: 0,
            health: SurfaceHealth::Disconnected,
            last_inbound: None,
            probe_deadline: None,
            counters: SurfaceCounters {
                sent: 0,
                superseded: 0,
                touch_suppressed: 0,
                resyncs: 0,
                reserved: 0,
                probes: 0,
            },
        }
    }

    /// The surface this controller speaks to.
    #[must_use]
    pub const fn profile(&self) -> &McuProfile {
        &self.profile
    }

    /// The clocks the outbound path runs on.
    #[must_use]
    pub const fn timing(&self) -> SurfaceTiming {
        self.timing
    }

    /// How much of the surface is PrismDMX's.
    #[must_use]
    pub const fn mode(&self) -> SurfaceMode {
        self.mode
    }

    /// The acceleration curves, so a settings screen can show what it is
    /// changing.
    #[must_use]
    pub const fn curves(&self) -> (VPotAcceleration, JogAcceleration) {
        (self.vpot, self.jog)
    }

    /// Replaces the acceleration curves.
    pub const fn set_curves(&mut self, vpot: VPotAcceleration, jog: JogAcceleration) {
        self.vpot = vpot;
        self.jog = jog;
    }

    /// Says how much of the surface PrismDMX now holds.
    ///
    /// A mode change invalidates the whole shadow model, because whatever the
    /// other host was showing is what is on the panel now — believing our own
    /// last picture would leave the desk drawing somebody else's.
    pub fn set_mode(&mut self, mode: SurfaceMode) {
        if mode == self.mode {
            return;
        }
        self.mode = mode;
        self.invalidate();
    }

    /// What is known about the surface at the other end.
    #[must_use]
    pub const fn health(&self) -> SurfaceHealth {
        self.health
    }

    /// What layer 2 has done and not done.
    #[must_use]
    pub const fn counters(&self) -> SurfaceCounters {
        self.counters
    }

    /// What layer 1 has counted underneath it.
    #[must_use]
    pub const fn codec_counters(&self) -> CodecCounters {
        self.codec.counters()
    }

    /// The picture the show wants shown.
    #[must_use]
    pub const fn desired(&self) -> &SurfaceState {
        &self.desired
    }

    /// The picture the desk was last given — the shadow model of
    /// `docs/MCU_MAPPING.md` §3.
    #[must_use]
    pub const fn shadow(&self) -> &SurfaceState {
        &self.shadow
    }

    /// Whether a hand is on a fader.
    #[must_use]
    pub fn is_touched(&self, fader: Fader) -> bool {
        fader_index(fader).is_some_and(|index| self.touched.get(index).copied().unwrap_or(false))
    }

    /// Whether PrismDMX may drive a control in the current mode.
    #[must_use]
    pub fn drives(&self, control: Control) -> bool {
        self.profile.holds(control, self.mode)
    }

    /// Messages this frame's diff found and has not sent yet.
    #[must_use]
    pub const fn pending(&self) -> usize {
        self.queued - self.cursor
    }

    // ---------------------------------------------------------------- lifecycle

    /// A surface has been opened, or reopened.
    ///
    /// Invalidates the shadow model, so the next frames transmit the whole
    /// picture — §5.3's resync burst, rate-limited by the same queue as
    /// everything else, because the burst is also the traffic pattern most
    /// likely to *cause* the fault of §2.7.
    pub fn connected(&mut self, now: Duration) {
        self.health = SurfaceHealth::Connected;
        self.last_inbound = None;
        self.probe_deadline = None;
        self.next_frame_at = now.saturating_add(self.timing.frame);
        self.next_send_at = now;
        self.invalidate();
    }

    /// The surface has gone.
    ///
    /// `docs/MCU_MAPPING.md` §5.3: this is a hardware fault like any other. The
    /// engine is not told, nothing is retried here, and the picture goes on being
    /// maintained so that whatever the show did meanwhile is on the desk when it
    /// comes back.
    pub fn disconnected(&mut self) {
        self.health = SurfaceHealth::Disconnected;
        self.last_inbound = None;
        self.probe_deadline = None;
        self.touched = [false; MAX_FADERS];
        self.resync_at = [None; MAX_FADERS];
        self.last_jog = None;
        self.invalidate();
    }

    /// Forgets everything believed about the desk, without touching what the
    /// show wants shown.
    fn invalidate(&mut self) {
        self.known = Known::nothing();
        self.queued = 0;
        self.cursor = 0;
    }

    // ------------------------------------------------------------------ inbound

    /// Feeds bytes from the port, calling `sink` once per interpreted event.
    ///
    /// `now` is the arrival time of this packet. Allocates nothing, whatever the
    /// bytes are, and a malformed packet reaches nobody — layer 1 counts it
    /// ([`codec_counters`](Self::codec_counters)).
    pub fn push<F>(&mut self, bytes: &[u8], now: Duration, mut sink: F)
    where
        F: FnMut(SurfaceEvent),
    {
        let before = self.codec.counters().wire.messages;
        {
            // Each field is bound on its own so the closure can hold what it
            // needs while the codec is borrowed mutably.
            let profile = self.profile;
            let timing = self.timing;
            let vpot = self.vpot;
            let jog = self.jog;
            let touched = &mut self.touched;
            let resync_at = &mut self.resync_at;
            let known = &mut self.known;
            let last_jog = &mut self.last_jog;
            let counters = &mut self.counters;
            self.codec.push(bytes, now, |event| match event {
                ControlEvent::Button { button, pressed } => {
                    if let ButtonId::Global(global) = button
                        && profile.is_reserved(global)
                    {
                        counters.reserved += 1;
                        return;
                    }
                    sink(SurfaceEvent::Button { button, pressed });
                }
                ControlEvent::Touch {
                    fader,
                    touched: now_touched,
                } => {
                    if let Some(index) = fader_index(fader) {
                        if let Some(slot) = touched.get_mut(index) {
                            *slot = now_touched;
                        }
                        // A hand on the fader means the motor is where the
                        // operator put it, not where we last drove it. Forgetting
                        // is what makes the resynchronisation below exactly one
                        // message rather than none.
                        known.faders = without_bit(known.faders, index);
                        if let Some(slot) = resync_at.get_mut(index) {
                            *slot = if now_touched {
                                None
                            } else {
                                Some(now.saturating_add(timing.touch_resync))
                            };
                        }
                    }
                    sink(SurfaceEvent::Touch {
                        fader,
                        touched: now_touched,
                    });
                }
                ControlEvent::Move { fader, position } => sink(SurfaceEvent::Moved {
                    fader,
                    level: profile.level_from_position(position),
                }),
                ControlEvent::VPot { strip, steps } => sink(SurfaceEvent::Encoder {
                    strip,
                    steps: vpot.steps(steps),
                }),
                ControlEvent::Jog { steps } => {
                    let since = last_jog.map(|previous| now.saturating_sub(previous));
                    *last_jog = Some(now);
                    sink(SurfaceEvent::Jog {
                        steps: jog.steps(steps, since),
                    });
                }
            });
        }
        if self.codec.counters().wire.messages > before {
            self.heard(now);
        }
    }

    /// Records that the surface said something.
    fn heard(&mut self, now: Duration) {
        if !self.health.is_attached() {
            return;
        }
        self.last_inbound = Some(now);
        self.probe_deadline = None;
        self.health = SurfaceHealth::Live;
    }

    // ----------------------------------------------------------------- outbound

    /// Sets a fader's level, 0…65535.
    pub fn set_fader(&mut self, fader: Fader, level: u16) {
        let Some(index) = fader_index(fader) else {
            return;
        };
        let changed = self.desired.faders.get(index) != Some(&level);
        if changed && self.fader_dirty(index) {
            self.counters.superseded += 1;
        }
        if changed && self.touched.get(index).copied().unwrap_or(false) {
            self.counters.touch_suppressed += 1;
        }
        if let Some(slot) = self.desired.faders.get_mut(index) {
            *slot = level;
        }
    }

    /// Sets a button's LED.
    ///
    /// Silently does nothing for a button that has no lamp — Name/Value and
    /// SMPTE/Beats on this surface (§2.7) — and for a reserved one. A console
    /// that lit a lamp which does not exist would have feedback that quietly
    /// lies about part of itself.
    pub fn set_led(&mut self, button: ButtonId, state: LedState) {
        match button {
            ButtonId::Strip {
                strip,
                button: which,
            } => {
                if self.profile.strip_note(strip, which).is_none() {
                    return;
                }
                let changed = self.desired_strip_led(strip, which) != Some(state);
                if changed && self.strip_led_dirty(strip, which) {
                    self.counters.superseded += 1;
                }
                if let Some(slot) = self
                    .desired
                    .strip_leds
                    .get_mut(usize::from(strip))
                    .and_then(|row| row.get_mut(which.index()))
                {
                    *slot = state;
                }
            }
            ButtonId::Global(which) => {
                if !self.profile.has_led(which) || self.profile.is_reserved(which) {
                    return;
                }
                let index = which.index();
                let changed = self.desired.global_leds.get(index) != Some(&state);
                if changed && self.global_led_dirty(which) {
                    self.counters.superseded += 1;
                }
                if let Some(slot) = self.desired.global_leds.get_mut(index) {
                    *slot = state;
                }
            }
        }
    }

    /// Sets a V-Pot's ring of LEDs.
    ///
    /// A position the mode cannot draw is clamped rather than refused: layer 1
    /// refuses it, so a caller's arithmetic error must not reach the encoder as
    /// a message that cannot be written. **The lamp under the encoder is never
    /// driven** — this surface has none (§2.7), so bit 6 of the ring value
    /// lights nothing.
    pub fn set_ring(&mut self, strip: u8, mode: RingMode, position: u8) {
        if strip >= self.profile.strips {
            return;
        }
        let ring = VPotRing {
            mode,
            position: position.min(mode.max_position()),
            led: false,
        };
        let index = usize::from(strip);
        let changed = self.desired.rings.get(index) != Some(&ring);
        if changed && self.ring_dirty(strip) {
            self.counters.superseded += 1;
        }
        if let Some(slot) = self.desired.rings.get_mut(index) {
            *slot = ring;
        }
    }

    /// Writes one line of one scribble strip.
    ///
    /// Seven characters, padded with spaces and truncated without ceremony.
    /// Anything the display cannot draw becomes `?`, and lower case is folded up
    /// the way the hardware folds it, so two spellings of one name are one state
    /// rather than two messages.
    pub fn set_text(&mut self, strip: u8, line: DisplayLine, text: &str) {
        if strip >= self.profile.strips {
            return;
        }
        let mut characters = [b' '; STRIP_CHARS];
        for (slot, character) in characters.iter_mut().zip(text.chars()) {
            *slot = display_byte(character);
        }
        let changed = self
            .desired
            .displays
            .get(usize::from(strip))
            .map(|display| *display.line(line))
            != Some(characters);
        if changed && self.text_dirty(strip, line) {
            self.counters.superseded += 1;
        }
        if let Some(slot) = self
            .desired
            .displays
            .get_mut(usize::from(strip))
            .and_then(|display| display.lines.get_mut(line.index()))
        {
            *slot = characters;
        }
    }

    /// Sets a scribble strip's backlight colour.
    ///
    /// One strip at a time here; **all eight on the wire**, because a colour
    /// message that does not carry exactly eight is ignored by the device
    /// (§2.7).
    pub fn set_color(&mut self, strip: u8, color: StripColor) {
        if strip >= self.profile.strips {
            return;
        }
        let index = usize::from(strip);
        let changed = self.desired.colors.get(index) != Some(&color);
        if changed && self.colors_dirty() {
            self.counters.superseded += 1;
        }
        if let Some(slot) = self.desired.colors.get_mut(index) {
            *slot = color;
        }
    }

    /// Sets a scribble strip's colour from an arbitrary one, quantised
    /// hue-first — see the `color` module.
    pub fn set_color_rgb(&mut self, strip: u8, color: RgbColor) {
        self.set_color(strip, quantize(color));
    }

    /// Sets a strip's level meter.
    ///
    /// A level above [`METER_LEVEL_OVER`] is clamped rather than refused: the
    /// two codes above it set and clear the overload marker, and a meter that
    /// was merely loud must not light one.
    pub fn set_meter(&mut self, strip: u8, signal: MeterSignal) {
        if strip >= self.profile.strips {
            return;
        }
        let signal = match signal {
            MeterSignal::Level(level) => MeterSignal::Level(level.min(METER_LEVEL_OVER)),
            overload @ MeterSignal::Overload(_) => overload,
        };
        let index = usize::from(strip);
        let changed = self.desired.meters.get(index) != Some(&signal);
        if changed && self.meter_dirty(strip) {
            self.counters.superseded += 1;
        }
        if let Some(slot) = self.desired.meters.get_mut(index) {
            *slot = signal;
        }
    }

    /// Sets one 7-segment digit, counted **from the right**.
    pub fn set_segment(&mut self, digit: u8, character: SegmentChar) {
        if digit >= self.profile.segments {
            return;
        }
        let character = SegmentChar {
            code: character.code & 0x3F,
            dot: character.dot,
        };
        let index = usize::from(digit);
        let changed = self.desired.segments.get(index) != Some(&character);
        if changed && self.segment_dirty(digit) {
            self.counters.superseded += 1;
        }
        if let Some(slot) = self.desired.segments.get_mut(index) {
            *slot = character;
        }
    }

    /// Writes a string across the 7-segment display, **right-aligned**.
    ///
    /// Digit 0 is the rightmost one, so the last character of the string lands
    /// there and the display reads left to right the way the string does — which
    /// is the whole reason this exists rather than being left to callers with a
    /// loop and an off-by-one.
    pub fn set_segment_text(&mut self, text: &str) {
        let mut digit = 0u8;
        for character in text.chars().rev() {
            if digit >= self.profile.segments {
                break;
            }
            let drawn = SegmentChar::from_ascii(display_byte(character)).unwrap_or(SegmentChar {
                code: 0,
                dot: false,
            });
            self.set_segment(digit, drawn);
            digit = digit.saturating_add(1);
        }
        while digit < self.profile.segments {
            self.set_segment(
                digit,
                SegmentChar {
                    code: 0,
                    dot: false,
                },
            );
            digit = digit.saturating_add(1);
        }
    }

    // --------------------------------------------------------------------- pump

    /// Sends what the desk is owed, subject to every rule in §5.
    ///
    /// Call it as often as the surface thread can — the schedule is the clock's,
    /// not the call count's, so pumping more often costs nothing and pumping less
    /// often sends less. Returns how many messages went out.
    pub fn pump<F>(&mut self, now: Duration, mut sink: F) -> usize
    where
        F: FnMut(Feedback<'_>),
    {
        if !self.health.is_attached() {
            return 0;
        }
        self.codec.poll(now);
        self.check_silence(now);
        let mut sent = 0;
        if self.probe_due(now) {
            sink(Feedback::DeviceQuery);
            self.counters.probes += 1;
            self.counters.sent += 1;
            self.health = SurfaceHealth::Probing;
            self.probe_deadline = Some(now.saturating_add(self.timing.probe));
            self.next_send_at = now.saturating_add(self.timing.min_gap);
            sent += 1;
        }
        if now >= self.next_frame_at {
            self.rebuild(now);
            self.next_frame_at = now.saturating_add(self.timing.frame);
        }
        while now >= self.next_send_at {
            // One condition rather than two: *is there another item* is the
            // same question as *is the cursor inside the queue*, and asking it
            // twice would leave one of the two answers untested.
            let Some(item) = self
                .queue
                .get(..self.queued)
                .and_then(|queued| queued.get(self.cursor))
                .copied()
            else {
                break;
            };
            self.cursor += 1;
            if self.emit(item, &mut sink) {
                self.counters.sent += 1;
                self.next_send_at = now.saturating_add(self.timing.min_gap);
                sent += 1;
            }
        }
        sent
    }

    /// Moves a desk that has gone quiet along its health states.
    fn check_silence(&mut self, now: Duration) {
        if let (SurfaceHealth::Probing, Some(deadline)) = (self.health, self.probe_deadline)
            && now >= deadline
        {
            self.health = SurfaceHealth::Unresponsive;
            self.probe_deadline = None;
        }
    }

    /// Whether to ask the surface, once, whether it is still there.
    ///
    /// Only a desk that **was** talking, only when the send queue is empty, and
    /// only once — after which the health is either [`SurfaceHealth::Live`]
    /// again or [`SurfaceHealth::Unresponsive`], and neither asks again. §2.7:
    /// the failure needs replies in flight, so this is the opposite of polling.
    fn probe_due(&self, now: Duration) -> bool {
        if self.health != SurfaceHealth::Live
            || self.cursor < self.queued
            || now < self.next_send_at
        {
            return false;
        }
        self.last_inbound
            .is_some_and(|last| now.saturating_sub(last) >= self.timing.silence)
    }

    /// Walks the difference between the two pictures, in the order §5.2 gives.
    fn rebuild(&mut self, now: Duration) {
        self.queued = 0;
        self.cursor = 0;

        // 1 - motor faders.
        for (index, fader) in ALL_FADERS.into_iter().enumerate() {
            if !self.drives(Control::Fader(fader)) || !self.fader_ready(index, now) {
                continue;
            }
            if self.fader_dirty(index) {
                self.enqueue(Pending::Fader(fader));
            }
        }

        // 2 - button LEDs, then the rings, which are LEDs by another name.
        for strip in 0..MAX_STRIPS {
            let strip = strip as u8;
            for button in StripButton::ALL {
                if !self.drives(Control::Button(ButtonId::Strip { strip, button })) {
                    continue;
                }
                if self.strip_led_dirty(strip, button) {
                    self.enqueue(Pending::StripLed(strip, button));
                }
            }
        }
        for button in GlobalButton::ALL {
            if !self.profile.has_led(button)
                || self.profile.is_reserved(button)
                || !self.drives(Control::Button(ButtonId::Global(button)))
            {
                continue;
            }
            if self.global_led_dirty(button) {
                self.enqueue(Pending::GlobalLed(button));
            }
        }
        for strip in 0..MAX_STRIPS {
            let strip = strip as u8;
            if !self.drives(Control::Encoder(strip)) {
                continue;
            }
            if self.ring_dirty(strip) {
                self.enqueue(Pending::Ring(strip));
            }
        }

        // 3 - the scribble strips, whose text and colour are separate state on
        //     the device (§2.7), and the 7-segment display.
        for strip in 0..MAX_STRIPS {
            let strip = strip as u8;
            if !self.drives(Control::Display(strip)) {
                continue;
            }
            for line in DisplayLine::ALL {
                if self.text_dirty(strip, line) {
                    self.enqueue(Pending::Text(strip, line));
                }
            }
        }
        if self.colors_drivable() && self.colors_dirty() {
            self.enqueue(Pending::Colors);
        }
        for digit in 0..MAX_SEGMENTS {
            let digit = digit as u8;
            if !self.drives(Control::Segment(digit)) {
                continue;
            }
            if self.segment_dirty(digit) {
                self.enqueue(Pending::Segment(digit));
            }
        }

        // 4 - meters, last and therefore first to be dropped.
        for strip in 0..MAX_STRIPS {
            let strip = strip as u8;
            if !self.drives(Control::Meter(strip)) {
                continue;
            }
            if self.meter_dirty(strip) {
                self.enqueue(Pending::Meter(strip));
            }
        }
    }

    /// Whether the colour message may go out at all.
    ///
    /// One message for the whole row, so it needs the whole row: in shared
    /// operation the strips belong to the sound console and the message would be
    /// writing over its display.
    fn colors_drivable(&self) -> bool {
        (0..MAX_STRIPS).all(|strip| self.drives(Control::Display(strip as u8)))
    }

    /// Whether a fader may be driven at this instant — §5.1.
    fn fader_ready(&self, index: usize, now: Duration) -> bool {
        if self.touched.get(index).copied().unwrap_or(false) {
            return false;
        }
        match self.resync_at.get(index).copied().flatten() {
            Some(deadline) => now >= deadline,
            None => true,
        }
    }

    // Seven questions of the same shape: *is what the show wants different from
    // what the desk was last told — or is it simply not known?* The second half
    // is what makes a connect, a reconnect and a mode change all produce a full
    // resync without any of them being written down as a special case, and it is
    // why these are one function each rather than an inline comparison at the
    // two places that ask.

    /// Whether a fader's position differs from what the desk was last told.
    fn fader_dirty(&self, index: usize) -> bool {
        !bit(self.known.faders, index)
            || self.desired.faders.get(index) != self.shadow.faders.get(index)
    }

    /// A strip button LED's state in the picture the show wants.
    fn desired_strip_led(&self, strip: u8, button: StripButton) -> Option<LedState> {
        self.desired
            .strip_leds
            .get(usize::from(strip))
            .and_then(|row| row.get(button.index()))
            .copied()
    }

    /// Whether a strip button's LED differs from what the desk was last told.
    fn strip_led_dirty(&self, strip: u8, button: StripButton) -> bool {
        !bit(self.known.strip_leds, Self::strip_led_index(strip, button))
            || self.desired_strip_led(strip, button)
                != self
                    .shadow
                    .strip_leds
                    .get(usize::from(strip))
                    .and_then(|row| row.get(button.index()))
                    .copied()
    }

    /// Whether a panel button's LED differs from what the desk was last told.
    fn global_led_dirty(&self, button: GlobalButton) -> bool {
        let index = button.index();
        !bit(self.known.global_leds, index)
            || self.desired.global_leds.get(index) != self.shadow.global_leds.get(index)
    }

    /// Whether a V-Pot's ring differs from what the desk was last told.
    fn ring_dirty(&self, strip: u8) -> bool {
        let index = usize::from(strip);
        !bit(self.known.rings, index)
            || self.desired.rings.get(index) != self.shadow.rings.get(index)
    }

    /// Whether a scribble strip line differs from what the desk was last told.
    fn text_dirty(&self, strip: u8, line: DisplayLine) -> bool {
        !bit(self.known.text, Self::text_index(strip, line))
            || self
                .desired
                .displays
                .get(usize::from(strip))
                .map(|display| display.line(line))
                != self
                    .shadow
                    .displays
                    .get(usize::from(strip))
                    .map(|display| display.line(line))
    }

    /// Whether any scribble strip colour differs. One question for all eight,
    /// because the message is all eight or nothing (§2.7).
    fn colors_dirty(&self) -> bool {
        !self.known.colors || self.desired.colors != self.shadow.colors
    }

    /// Whether a meter differs from what the desk was last told.
    fn meter_dirty(&self, strip: u8) -> bool {
        let index = usize::from(strip);
        !bit(self.known.meters, index)
            || self.desired.meters.get(index) != self.shadow.meters.get(index)
    }

    /// Whether a 7-segment digit differs from what the desk was last told.
    fn segment_dirty(&self, digit: u8) -> bool {
        let index = usize::from(digit);
        !bit(self.known.segments, index)
            || self.desired.segments.get(index) != self.shadow.segments.get(index)
    }

    /// Adds an item to this frame's queue, or drops it if the queue is full —
    /// in which case it is still different at the next frame, because the shadow
    /// only moves when a message goes out.
    fn enqueue(&mut self, item: Pending) {
        if let Some(slot) = self.queue.get_mut(self.queued) {
            *slot = item;
            self.queued += 1;
        }
    }

    /// Sends one item and moves the shadow model up to it.
    fn emit<F>(&mut self, item: Pending, sink: &mut F) -> bool
    where
        F: FnMut(Feedback<'_>),
    {
        match item {
            Pending::Fader(fader) => {
                let Some(index) = fader_index(fader) else {
                    return false;
                };
                let Some(level) = self.desired.faders.get(index).copied() else {
                    return false;
                };
                sink(Feedback::Move {
                    fader,
                    position: self.profile.position_from_level(level),
                });
                if let Some(slot) = self.shadow.faders.get_mut(index) {
                    *slot = level;
                }
                self.known.faders = with_bit(self.known.faders, index);
                if let Some(slot) = self.resync_at.get_mut(index)
                    && slot.is_some()
                {
                    *slot = None;
                    self.counters.resyncs += 1;
                }
                true
            }
            Pending::StripLed(strip, button) => {
                let Some(state) = self
                    .desired
                    .strip_leds
                    .get(usize::from(strip))
                    .and_then(|row| row.get(button.index()))
                    .copied()
                else {
                    return false;
                };
                sink(Feedback::Led {
                    button: ButtonId::Strip { strip, button },
                    state,
                });
                if let Some(slot) = self
                    .shadow
                    .strip_leds
                    .get_mut(usize::from(strip))
                    .and_then(|row| row.get_mut(button.index()))
                {
                    *slot = state;
                }
                self.known.strip_leds =
                    with_bit(self.known.strip_leds, Self::strip_led_index(strip, button));
                true
            }
            Pending::GlobalLed(button) => {
                let index = button.index();
                let Some(state) = self.desired.global_leds.get(index).copied() else {
                    return false;
                };
                sink(Feedback::Led {
                    button: ButtonId::Global(button),
                    state,
                });
                if let Some(slot) = self.shadow.global_leds.get_mut(index) {
                    *slot = state;
                }
                self.known.global_leds = with_bit(self.known.global_leds, index);
                true
            }
            Pending::Ring(strip) => {
                let index = usize::from(strip);
                let Some(ring) = self.desired.rings.get(index).copied() else {
                    return false;
                };
                sink(Feedback::Ring { strip, ring });
                if let Some(slot) = self.shadow.rings.get_mut(index) {
                    *slot = ring;
                }
                self.known.rings = with_bit(self.known.rings, index);
                true
            }
            Pending::Text(strip, line) => {
                let Some(display) = self.desired.displays.get(usize::from(strip)).copied() else {
                    return false;
                };
                let base = match line {
                    DisplayLine::Upper => 0,
                    DisplayLine::Lower => self.profile.lcd_line_offset,
                };
                let Some(offset) = self
                    .profile
                    .lcd_chars_per_strip
                    .checked_mul(strip)
                    .and_then(|start| start.checked_add(base))
                else {
                    return false;
                };
                sink(Feedback::DisplayText {
                    offset,
                    text: display.line(line),
                });
                if let Some(slot) = self
                    .shadow
                    .displays
                    .get_mut(usize::from(strip))
                    .and_then(|shadow| shadow.lines.get_mut(line.index()))
                    && let Some(sent) = display.lines.get(line.index())
                {
                    *slot = *sent;
                }
                self.known.text = with_bit(self.known.text, Self::text_index(strip, line));
                true
            }
            Pending::Colors => {
                sink(Feedback::DisplayColors(self.desired.colors));
                self.shadow.colors = self.desired.colors;
                self.known.colors = true;
                true
            }
            Pending::Segment(digit) => {
                let index = usize::from(digit);
                let Some(character) = self.desired.segments.get(index).copied() else {
                    return false;
                };
                sink(Feedback::Segment { digit, character });
                if let Some(slot) = self.shadow.segments.get_mut(index) {
                    *slot = character;
                }
                self.known.segments = with_bit(self.known.segments, index);
                true
            }
            Pending::Meter(strip) => {
                let index = usize::from(strip);
                let Some(signal) = self.desired.meters.get(index).copied() else {
                    return false;
                };
                sink(Feedback::Meter { strip, signal });
                if let Some(slot) = self.shadow.meters.get_mut(index) {
                    *slot = signal;
                }
                self.known.meters = with_bit(self.known.meters, index);
                true
            }
        }
    }

    /// Where a strip button's LED sits in the shadow model.
    const fn strip_led_index(strip: u8, button: StripButton) -> usize {
        (strip as usize) * STRIP_BUTTONS + button.index()
    }

    /// Where a scribble strip line sits in the shadow model.
    const fn text_index(strip: u8, line: DisplayLine) -> usize {
        (strip as usize) * 2 + line.index()
    }
}

/// The byte a character is written as, or `?` for one the display cannot draw.
///
/// Folded to upper case because the hardware folds it anyway (§2.2), so holding
/// the folded form keeps two spellings of one name from being two states.
fn display_byte(character: char) -> u8 {
    if !character.is_ascii() {
        return b'?';
    }
    let byte = (character as u8).to_ascii_uppercase();
    if (0x20..=0x7E).contains(&byte) {
        byte
    } else {
        b'?'
    }
}

/// Compile-time proof that one frame can hold the whole surface at once.
///
/// A resync burst is every control there is, and a queue that silently dropped
/// the tail of it would leave a desk part-drawn until something else happened to
/// change — the kind of fault that only shows up on the night somebody
/// power-cycles the console mid-show.
const _: () = {
    let everything = MAX_FADERS
        + MAX_STRIPS * STRIP_BUTTONS
        + GLOBAL_BUTTONS
        + MAX_STRIPS
        + MAX_STRIPS * DISPLAY_LINES
        + 1
        + MAX_SEGMENTS
        + MAX_STRIPS;
    assert!(QUEUE_CAPACITY >= everything);
};

#[cfg(test)]
mod tests {
    use super::{
        QUEUE_CAPACITY, SurfaceController, SurfaceHealth, SurfaceTiming, bit, display_byte,
        with_bit, without_bit,
    };
    use crate::accel::VPotAcceleration;
    use crate::control::ButtonId;
    use crate::feedback::{
        Feedback, LedState, METER_LEVEL_OVER, MeterSignal, RingMode, SegmentChar, StripColor,
    };
    use crate::midi::MAX_MESSAGE_BYTES;
    use crate::model::{Control, DisplayLine, Priority, SurfaceEvent, SurfaceMode, fader_at};
    use crate::profile::{Fader, GlobalButton, StripButton, X_TOUCH};
    use prism_domain::RgbColor;
    use std::time::Duration;

    /// One outbound message, kept as the bytes it would put on the wire.
    ///
    /// Owned on purpose: a `Feedback` borrows the state it was built from, and a
    /// test that held one would be asserting about a picture that had moved on.
    /// Encoding is also the strongest form of one claim — **every message this
    /// layer produces has to be one layer 1 can write** — so a layer 2 that
    /// asked for a ninth strip is caught here rather than on a desk.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Sent {
        bytes: Vec<u8>,
        priority: Option<Priority>,
    }

    fn record(feedback: &Feedback<'_>, out: &mut Vec<Sent>) {
        let mut buf = [0u8; MAX_MESSAGE_BYTES];
        let written = feedback
            .encode_into(&X_TOUCH, &mut buf)
            .expect("layer 2 must never produce a message layer 1 cannot write");
        out.push(Sent {
            bytes: buf.get(..written).expect("written fits").to_vec(),
            priority: Priority::of(feedback),
        });
    }

    /// Pumps once per millisecond from `from` to `until`, collecting everything
    /// sent. A millisecond is the minimum gap, so this is the fastest a caller
    /// is allowed to drive the surface.
    fn drain(controller: &mut SurfaceController, from: Duration, until: Duration) -> Vec<Sent> {
        let mut out = Vec::new();
        let mut now = from;
        while now <= until {
            controller.pump(now, |feedback| record(&feedback, &mut out));
            now = now.saturating_add(Duration::from_millis(1));
        }
        out
    }

    /// A controller with a surface attached at time zero and its resync burst
    /// already sent, which is where most of these tests want to start.
    fn settled() -> (SurfaceController, Duration) {
        let mut controller = SurfaceController::new(X_TOUCH);
        controller.connected(Duration::ZERO);
        let start = Duration::from_secs(1);
        let sent = drain(&mut controller, Duration::ZERO, start);
        assert!(!sent.is_empty(), "the burst has to have happened");
        assert_eq!(drain(&mut controller, start, start).len(), 0);
        (controller, start)
    }

    #[test]
    fn a_controller_with_no_surface_says_nothing_and_goes_on_taking_state() {
        // The engine is not told a desk is missing, and the picture goes on
        // being maintained - `docs/MCU_MAPPING.md` §5.3, and the exit criterion
        // that a device's disappearance leaves the engine alone.
        let mut controller = SurfaceController::new(X_TOUCH);
        assert_eq!(controller.health(), SurfaceHealth::Disconnected);
        controller.set_fader(Fader::Strip(0), u16::MAX);
        controller.set_text(0, DisplayLine::Upper, "WASH");
        assert_eq!(
            drain(&mut controller, Duration::ZERO, Duration::from_secs(1)).len(),
            0
        );
        assert_eq!(controller.counters().sent, 0);
        assert_eq!(controller.desired().fader(Fader::Strip(0)), Some(u16::MAX));
        assert_eq!(
            controller.health().remedy(),
            Some("no surface is connected")
        );
    }

    #[test]
    fn connecting_draws_the_whole_surface_once_and_then_stops() {
        // The resync burst of §5.3: every control the shadow model has, exactly
        // once, and nothing afterwards. 156 is nine faders, forty strip LEDs,
        // sixty-two lit panel buttons, eight rings, sixteen lines of text, one
        // colour message, twelve digits and eight meters - added up by hand here,
        // because a figure the controller computed would agree with any answer.
        let mut controller = SurfaceController::new(X_TOUCH);
        controller.connected(Duration::ZERO);
        let sent = drain(&mut controller, Duration::ZERO, Duration::from_secs(1));
        assert_eq!(sent.len(), 9 + 40 + 62 + 8 + 16 + 1 + 12 + 8);
        assert_eq!(sent.len(), 156);
        assert!(QUEUE_CAPACITY >= sent.len());
        // Once each: a burst that sent a control twice would be a shadow model
        // that does not believe its own messages.
        let mut seen: Vec<&[u8]> = sent
            .iter()
            .map(|message| message.bytes.as_slice())
            .collect();
        seen.sort_unstable();
        let before = seen.len();
        seen.dedup();
        assert_eq!(seen.len(), before, "a control was drawn twice");
        assert_eq!(controller.counters().sent, 156);
        assert_eq!(controller.pending(), 0);
    }

    #[test]
    fn the_burst_is_paced_rather_than_poured_into_the_port() {
        // The fault of §2.7 is what this is about: 156 messages as fast as the
        // port accepts them is the traffic pattern that can stop the surface
        // transmitting until somebody power-cycles it. Two claims - never two
        // messages at one instant, and never two closer than the minimum gap.
        let mut controller = SurfaceController::new(X_TOUCH);
        let timing = controller.timing();
        controller.connected(Duration::ZERO);
        let mut now = Duration::ZERO;
        let mut last: Option<Duration> = None;
        let mut total = 0;
        while now <= Duration::from_secs(1) {
            let sent = controller.pump(now, |_| {});
            assert!(sent <= 1, "{sent} messages went out at the same instant");
            if sent == 1 {
                if let Some(previous) = last {
                    assert!(
                        now.saturating_sub(previous) >= timing.min_gap,
                        "two messages {:?} apart",
                        now.saturating_sub(previous)
                    );
                }
                last = Some(now);
                total += 1;
            }
            now = now.saturating_add(Duration::from_micros(250));
        }
        assert_eq!(total, 156);
    }

    #[test]
    fn nothing_is_sent_twice_when_nothing_has_changed() {
        // The whole point of diffing. A layer that re-sent its picture every
        // frame would put 156 messages a frame on a port S20 showed can be
        // talked to death.
        let (mut controller, start) = settled();
        assert_eq!(
            drain(&mut controller, start, start + Duration::from_secs(2)).len(),
            0
        );
    }

    #[test]
    fn a_ring_never_lights_the_lamp_this_surface_has_not_got() {
        // §2.7: the encoders have no lamp under them, so bit 6 of a ring value
        // lights nothing. Asserted on the byte rather than on the type, because
        // the byte is what the desk sees.
        let (mut controller, start) = settled();
        for strip in 0..8u8 {
            controller.set_ring(strip, RingMode::Wrap, 11);
        }
        let sent = drain(&mut controller, start, start + Duration::from_millis(200));
        assert_eq!(sent.len(), 8);
        for message in sent {
            let value = *message.bytes.get(2).expect("a control change has a value");
            assert_eq!(
                value & 0x40,
                0,
                "the encoder lamp bit was set: {value:#04X}"
            );
        }
    }

    #[test]
    fn a_ring_position_the_mode_cannot_draw_is_clamped_rather_than_dropped() {
        // Layer 1 refuses it. A caller's arithmetic error must not become a
        // message that cannot be written, and must not become silence either.
        let (mut controller, start) = settled();
        controller.set_ring(0, RingMode::Spread, 11);
        assert_eq!(
            controller.desired().ring(0).map(|ring| ring.position),
            Some(6)
        );
        let sent = drain(&mut controller, start, start + Duration::from_millis(200));
        assert_eq!(sent.len(), 1);
    }

    #[test]
    fn the_two_buttons_with_no_lamp_are_never_driven() {
        // §2.7: Name/Value and SMPTE/Beats send their notes and stay dark. A
        // console that drove them would have feedback that quietly lies about
        // part of itself.
        let (mut controller, start) = settled();
        for button in [GlobalButton::NameValue, GlobalButton::SmpteBeats] {
            controller.set_led(ButtonId::Global(button), LedState::On);
            assert_eq!(
                controller.desired().led(ButtonId::Global(button)),
                Some(LedState::Off),
                "{button:?} took a state it cannot show"
            );
        }
        assert_eq!(
            drain(&mut controller, start, start + Duration::from_millis(200)).len(),
            0
        );
    }

    #[test]
    fn a_press_of_the_reserved_button_never_reaches_anybody() {
        // §4.3: SMPTE/Beats switches the surface between the two hosts, so it is
        // the operator's way back to their sound desk. Dropped here, and counted,
        // so that nothing above is ever given the chance to claim it.
        let (mut controller, start) = settled();
        let mut events = Vec::new();
        controller.push(&[0x90, 53, 0x7F], start, |event| events.push(event));
        controller.push(&[0x90, 53, 0x00], start, |event| events.push(event));
        assert!(events.is_empty());
        assert_eq!(controller.counters().reserved, 2);
        // Layer 1 still saw them, so a desk sending nothing at all and a desk
        // sending a reserved button are different questions.
        assert_eq!(controller.codec_counters().wire.messages, 2);
        assert_eq!(controller.codec_counters().discarded(), 0);
    }

    #[test]
    fn a_fader_arrives_as_a_level_scaled_against_the_top_of_travel() {
        // `E0 7C 7F` is the top-of-travel message S20 recorded: 124 + 128 x 127
        // = 16380. Transcribed rather than computed, because a test that asked
        // the profile the same question the code asks it would agree with any
        // answer - and 16380 against 16383 is the difference between a master
        // that reaches full and one that stops at 99.98 %.
        let (mut controller, start) = settled();
        let mut events = Vec::new();
        controller.push(&[0xE0, 0x7C, 0x7F], start, |event| events.push(event));
        assert_eq!(
            events,
            vec![SurfaceEvent::Moved {
                fader: Fader::Strip(0),
                level: u16::MAX
            }]
        );
        // And the bottom, which no scaling gets wrong but every off-by-one does.
        events.clear();
        controller.push(&[0xE0, 0x00, 0x00], start, |event| events.push(event));
        assert_eq!(
            events,
            vec![SurfaceEvent::Moved {
                fader: Fader::Strip(0),
                level: 0
            }]
        );
    }

    #[test]
    fn a_level_of_full_parks_the_motor_where_the_desk_reports_full() {
        // The same number in the other direction, asserted on the bytes: a
        // master at full has to put the fader exactly where the surface itself
        // says the top is, or the desk reports 99.98 % straight back.
        let (mut controller, start) = settled();
        controller.set_fader(Fader::Strip(0), u16::MAX);
        let sent = drain(&mut controller, start, start + Duration::from_millis(200));
        assert_eq!(sent.len(), 1);
        assert_eq!(
            sent.first().map(|message| message.bytes.as_slice()),
            Some(&[0xE0, 0x7C, 0x7F][..])
        );
    }

    #[test]
    fn the_two_acceleration_curves_reach_the_events() {
        // The V-Pot's magnitude and the jog wheel's clock, through the
        // controller rather than through the curves on their own - the wiring is
        // the part that can be crossed over.
        let (mut controller, start) = settled();
        let mut events = Vec::new();
        controller.push(&[0xB0, 16, 0x08], start, |event| events.push(event));
        assert_eq!(
            events,
            vec![SurfaceEvent::Encoder {
                strip: 0,
                steps: 36
            }]
        );
        events.clear();
        // The first jog message after a pause is a click, not a spin.
        controller.push(&[0xB0, 60, 0x01], start, |event| events.push(event));
        assert_eq!(events, vec![SurfaceEvent::Jog { steps: 1 }]);
        events.clear();
        // The same magnitude five milliseconds later is a spin.
        controller.push(
            &[0xB0, 60, 0x01],
            start + Duration::from_millis(5),
            |event| {
                events.push(event);
            },
        );
        assert_eq!(events, vec![SurfaceEvent::Jog { steps: 8 }]);
    }

    #[test]
    fn text_and_colour_are_separate_state_on_the_way_out() {
        // §2.7, measured at the desk: writing text does not disturb the colour.
        // So they diff apart, and a name changing does not cost the colour
        // message as well.
        let (mut controller, start) = settled();
        controller.set_text(3, DisplayLine::Upper, "CYC");
        let sent = drain(&mut controller, start, start + Duration::from_millis(200));
        assert_eq!(sent.len(), 1);
        // Strip 3's upper line starts at 21, and the payload is seven characters
        // padded with spaces - written out by hand.
        assert_eq!(
            sent.first().map(|message| message.bytes.as_slice()),
            Some(
                &[
                    0xF0, 0x00, 0x00, 0x66, 0x14, 0x12, 21, b'C', b'Y', b'C', b' ', b' ', b' ',
                    b' ', 0xF7
                ][..]
            )
        );
        // The colour, alone, is one message for all eight strips.
        let start = start + Duration::from_millis(200);
        controller.set_color(3, StripColor::Cyan);
        let sent = drain(&mut controller, start, start + Duration::from_millis(200));
        assert_eq!(sent.len(), 1);
        assert_eq!(
            sent.first().map(|message| message.bytes.as_slice()),
            Some(
                &[
                    0xF0, 0x00, 0x00, 0x66, 0x14, 0x72, 7, 7, 7, 6, 7, 7, 7, 7, 0xF7
                ][..]
            )
        );
    }

    #[test]
    fn text_is_padded_truncated_and_folded_the_way_the_display_shows_it() {
        let (mut controller, _) = settled();
        controller.set_text(0, DisplayLine::Upper, "cyc");
        assert_eq!(
            controller.desired().text(0, DisplayLine::Upper),
            Some(b"CYC    ")
        );
        controller.set_text(0, DisplayLine::Lower, "ELEVENCHARS");
        assert_eq!(
            controller.desired().text(0, DisplayLine::Lower),
            Some(b"ELEVENC")
        );
        // A character the display cannot draw is visible as a question mark
        // rather than as a hole only the desk can see.
        controller.set_text(1, DisplayLine::Upper, "GRUEN\u{00DC}");
        assert_eq!(
            controller.desired().text(1, DisplayLine::Upper),
            Some(b"GRUEN? ")
        );
    }

    #[test]
    fn the_seven_segment_display_is_written_right_aligned() {
        // Digit 0 is the rightmost (§2.7), so the last character of the string
        // goes there and the display reads the way the string does. This helper
        // exists to make that off-by-one once instead of at every call site.
        let (mut controller, _) = settled();
        controller.set_segment_text("42");
        assert_eq!(
            controller.desired().segment(0).map(SegmentChar::to_ascii),
            Some(b'2')
        );
        assert_eq!(
            controller.desired().segment(1).map(SegmentChar::to_ascii),
            Some(b'4')
        );
        for digit in 2..12u8 {
            assert_eq!(
                controller.desired().segment(digit).map(|c| c.code),
                Some(0),
                "digit {digit} was left holding something"
            );
        }
        // A string longer than the display loses its left end, not its right:
        // the digits that matter on a page number are the last ones.
        controller.set_segment_text("ABCDEFGHIJKLMN");
        assert_eq!(
            controller.desired().segment(0).map(SegmentChar::to_ascii),
            Some(b'N')
        );
        assert_eq!(
            controller.desired().segment(11).map(SegmentChar::to_ascii),
            Some(b'C')
        );
    }

    #[test]
    fn a_meter_level_that_would_light_an_overload_marker_is_clamped() {
        // 0x0E and 0x0F set and clear the marker. Layer 1 refuses a level that
        // lands on them; layer 2 must not hand it one.
        let (mut controller, start) = settled();
        controller.set_meter(0, MeterSignal::Level(0x2F));
        assert_eq!(
            controller.desired().meter(0),
            Some(MeterSignal::Level(METER_LEVEL_OVER))
        );
        let sent = drain(&mut controller, start, start + Duration::from_millis(200));
        assert_eq!(
            sent.first().map(|message| message.bytes.as_slice()),
            Some(&[0xD0, 0x0D][..])
        );
    }

    #[test]
    fn a_segment_code_wider_than_the_display_is_masked_rather_than_refused() {
        let (mut controller, start) = settled();
        controller.set_segment(
            0,
            SegmentChar {
                code: 0x7F,
                dot: true,
            },
        );
        assert_eq!(controller.desired().segment(0).map(|c| c.code), Some(0x3F));
        assert_eq!(
            drain(&mut controller, start, start + Duration::from_millis(200)).len(),
            1
        );
    }

    #[test]
    fn a_colour_arrives_through_the_quantiser() {
        let (mut controller, _) = settled();
        controller.set_color_rgb(
            0,
            RgbColor {
                r: 255,
                g: 200,
                b: 180,
            },
        );
        assert_eq!(controller.desired().color(0), Some(StripColor::Red));
        controller.set_color_rgb(1, RgbColor { r: 0, g: 0, b: 0 });
        assert_eq!(controller.desired().color(1), Some(StripColor::Off));
    }

    #[test]
    fn a_control_the_surface_has_not_got_is_ignored_by_every_setter() {
        // A paging fault upstairs asks for a ninth strip. Nothing must reach the
        // encoder, and nothing must panic - the crate denies both.
        let (mut controller, start) = settled();
        controller.set_fader(Fader::Strip(9), u16::MAX);
        controller.set_led(
            ButtonId::Strip {
                strip: 8,
                button: StripButton::Rec,
            },
            LedState::On,
        );
        controller.set_ring(8, RingMode::Dot, 1);
        controller.set_text(8, DisplayLine::Upper, "X");
        controller.set_color(8, StripColor::Red);
        controller.set_meter(8, MeterSignal::Level(1));
        controller.set_segment(
            12,
            SegmentChar {
                code: 1,
                dot: false,
            },
        );
        assert_eq!(
            drain(&mut controller, start, start + Duration::from_millis(200)).len(),
            0
        );
    }

    #[test]
    fn a_shared_surface_is_only_driven_where_it_is_ours() {
        // §4.3: in the combined Xctl+MC mode the strips belong to the sound
        // console. Lighting a Select LED there is at best ignored and at worst
        // fights that console's own feedback, so the shadow model does not.
        let mut controller = SurfaceController::new(X_TOUCH);
        controller.set_mode(SurfaceMode::Shared);
        controller.connected(Duration::ZERO);
        let sent = drain(&mut controller, Duration::ZERO, Duration::from_secs(1));
        // Five transport LEDs and nothing else - the jog wheel is inbound only.
        assert_eq!(sent.len(), 5);
        for message in &sent {
            let note = *message.bytes.get(1).expect("a note on has a note");
            assert!((91..=95).contains(&note), "note {note} is not ours");
        }
        // A strip's LED set while the sound console has the panel is remembered
        // and not sent...
        let start = Duration::from_secs(1);
        controller.set_led(
            ButtonId::Strip {
                strip: 2,
                button: StripButton::Select,
            },
            LedState::On,
        );
        assert_eq!(
            drain(&mut controller, start, start + Duration::from_millis(200)).len(),
            0
        );
        // ...and drawn when the operator switches the surface back, along with
        // everything else, because what is on the panel now is the other host's.
        let start = start + Duration::from_millis(200);
        controller.set_mode(SurfaceMode::Dedicated);
        let sent = drain(&mut controller, start, start + Duration::from_secs(1));
        assert_eq!(sent.len(), 156);
    }

    #[test]
    fn a_shared_surface_still_hears_everything_it_is_sent() {
        // Ownership is about *feedback*. If a control reaches us in the shared
        // mode at all, the operator pressed it and meant it - filtering inbound
        // events would be inventing a second rule the surface does not have.
        let mut controller = SurfaceController::new(X_TOUCH);
        controller.set_mode(SurfaceMode::Shared);
        controller.connected(Duration::ZERO);
        let mut events = Vec::new();
        controller.push(&[0x90, 24, 0x7F], Duration::ZERO, |event| {
            events.push(event)
        });
        assert_eq!(
            events,
            vec![SurfaceEvent::Button {
                button: ButtonId::Strip {
                    strip: 0,
                    button: StripButton::Select
                },
                pressed: true
            }]
        );
        assert!(!controller.drives(Control::Button(ButtonId::Strip {
            strip: 0,
            button: StripButton::Select
        })));
        assert!(controller.drives(Control::Button(ButtonId::Global(GlobalButton::Play))));
    }

    #[test]
    fn setting_the_mode_to_the_one_it_already_has_costs_nothing() {
        let (mut controller, start) = settled();
        controller.set_mode(SurfaceMode::Dedicated);
        assert_eq!(
            drain(&mut controller, start, start + Duration::from_millis(200)).len(),
            0
        );
    }

    #[test]
    fn coalescing_is_counted_so_a_frame_rate_can_be_argued_with() {
        // What the 30 Hz saved, as a number rather than as a claim. A change
        // that is overwritten before it is sent is a message that did not go to
        // a device S20 showed can be talked to death.
        let (mut controller, start) = settled();
        for level in 0..500u16 {
            controller.set_fader(Fader::Strip(1), level);
        }
        // 498 rather than 499: the first change had nothing to supersede, and
        // level 0 was not a change at all. What is counted is a value that
        // replaced one the desk had not been told about yet.
        assert_eq!(controller.counters().superseded, 498);
        let sent = drain(&mut controller, start, start + Duration::from_millis(200));
        assert_eq!(sent.len(), 1);
        // And a set that changes nothing is not a change.
        let before = controller.counters().superseded;
        controller.set_fader(Fader::Strip(1), 499);
        assert_eq!(controller.counters().superseded, before);
    }

    #[test]
    fn every_kind_of_message_survives_a_full_state_change() {
        // A picture where every control has moved at once: the case a diff
        // written control by control gets right and a diff written by class
        // gets wrong.
        let (mut controller, start) = settled();
        for strip in 0..8u8 {
            controller.set_fader(Fader::Strip(strip), u16::MAX / 2);
            controller.set_ring(strip, RingMode::BoostCut, 5);
            controller.set_meter(strip, MeterSignal::Level(6));
            controller.set_color(strip, StripColor::Blue);
            for line in DisplayLine::ALL {
                controller.set_text(strip, line, "AB");
            }
            for button in StripButton::ALL {
                controller.set_led(ButtonId::Strip { strip, button }, LedState::Flashing);
            }
        }
        controller.set_fader(Fader::Main, 1);
        // No space in it on purpose: a blank digit is what the display already
        // holds, and a test whose "change" was not one would be counting 11.
        controller.set_segment_text("PAGE-1");
        controller.set_led(ButtonId::Global(GlobalButton::Play), LedState::On);
        let sent = drain(&mut controller, start, start + Duration::from_secs(1));
        assert_eq!(sent.len(), 9 + 40 + 1 + 8 + 16 + 1 + 12 + 8);
        assert_eq!(controller.desired(), controller.shadow());
    }

    #[test]
    fn a_desk_that_was_talking_and_goes_quiet_is_asked_once_and_only_once() {
        // §2.7's third consequence. Never poll: the device query is the only
        // reply this surface generates, and the fault needs replies in flight.
        // So one question, and then an answer either way.
        let (mut controller, start) = settled();
        controller.push(&[0x90, 94, 0x7F], start, |_| {});
        assert_eq!(controller.health(), SurfaceHealth::Live);
        let timing = controller.timing();
        let sent = drain(
            &mut controller,
            start,
            start + timing.silence + timing.probe + Duration::from_secs(60),
        );
        assert_eq!(sent.len(), 1, "the desk was asked more than once");
        assert_eq!(
            sent.first().map(|message| message.bytes.as_slice()),
            Some(&[0xF0, 0x00, 0x00, 0x66, 0x14, 0x00, 0xF7][..])
        );
        assert_eq!(sent.first().and_then(|message| message.priority), None);
        assert_eq!(controller.counters().probes, 1);
        assert_eq!(controller.health(), SurfaceHealth::Unresponsive);
        assert_eq!(
            controller.health().remedy(),
            Some(
                "the surface has stopped sending: power-cycle it. \
                 Reopening the port or restarting will not bring it back"
            )
        );
    }

    #[test]
    fn a_desk_that_answers_is_live_again_and_is_not_asked_twice() {
        let (mut controller, start) = settled();
        controller.push(&[0x90, 94, 0x7F], start, |_| {});
        let timing = controller.timing();
        let asked = start + timing.silence + Duration::from_millis(1);
        assert_eq!(drain(&mut controller, start, asked).len(), 1);
        assert_eq!(controller.health(), SurfaceHealth::Probing);
        // The answer is a SysEx layer 1 counts and does not interpret, which is
        // exactly enough: what is being asked is whether anything comes back.
        controller.push(
            &[0xF0, 0x00, 0x00, 0x66, 0x14, 0x01, 0x30, 0xF7],
            asked + Duration::from_millis(1),
            |_| {},
        );
        assert_eq!(controller.health(), SurfaceHealth::Live);
        assert_eq!(controller.codec_counters().sysex_ignored, 1);
        let quiet = asked + timing.probe + Duration::from_secs(1);
        assert_eq!(drain(&mut controller, asked, quiet).len(), 0);
        assert_eq!(controller.counters().probes, 1);
    }

    #[test]
    fn a_desk_nobody_has_touched_yet_is_not_called_dead() {
        // An X-Touch speaks only when it is touched, so silence on its own means
        // nothing. The watchdog only ever suspects a desk that *was* talking -
        // §5.3's wording, and the reason `Connected` and `Live` are two states.
        let (mut controller, start) = settled();
        assert_eq!(controller.health(), SurfaceHealth::Connected);
        let timing = controller.timing();
        let sent = drain(
            &mut controller,
            start,
            start + timing.silence + timing.probe + Duration::from_secs(10),
        );
        assert_eq!(sent.len(), 0);
        assert_eq!(controller.health(), SurfaceHealth::Connected);
        assert_eq!(controller.counters().probes, 0);
    }

    #[test]
    fn the_question_waits_for_the_queue_to_be_empty() {
        // The recipe in §2.7 is a flood of writes *with replies outstanding*.
        // Asking while a burst is still going out would be building it.
        // A silence window short enough to expire in the middle of the resync
        // burst, which is the arrangement the rule is about. The burst is 156
        // messages a millisecond apart; the window is fifty.
        let timing = SurfaceTiming {
            silence: Duration::from_millis(50),
            ..SurfaceTiming::DEFAULT
        };
        let mut controller = SurfaceController::with_timing(X_TOUCH, timing);
        controller.connected(Duration::ZERO);
        controller.push(&[0x90, 94, 0x7F], Duration::ZERO, |_| {});
        let during = drain(&mut controller, Duration::ZERO, Duration::from_millis(60));
        assert!(
            controller.pending() > 0,
            "the burst has to still be running"
        );
        assert_eq!(queries(&during), 0);
        assert_eq!(controller.counters().probes, 0);
        // And once the burst is out, the question goes - once.
        let rest = drain(
            &mut controller,
            Duration::from_millis(60),
            Duration::from_secs(5),
        );
        assert_eq!(queries(&rest), 1);
        assert_eq!(controller.counters().probes, 1);
    }

    /// How many of these messages are the device query.
    fn queries(sent: &[Sent]) -> usize {
        sent.iter()
            .filter(|message| {
                message.bytes.first() == Some(&0xF0) && message.bytes.get(5) == Some(&0x00)
            })
            .count()
    }

    #[test]
    fn a_disconnected_surface_forgets_what_it_believed_and_the_show_goes_on() {
        // The exit criterion, from this side: the engine's picture is untouched
        // by the desk going away, and everything that happened meanwhile is on
        // the desk when it comes back.
        let (mut controller, start) = settled();
        controller.push(&[0x90, 104, 0x7F], start, |_| {});
        assert!(controller.is_touched(Fader::Strip(0)));
        controller.disconnected();
        assert_eq!(controller.health(), SurfaceHealth::Disconnected);
        assert!(!controller.is_touched(Fader::Strip(0)));
        controller.set_fader(Fader::Strip(4), u16::MAX);
        controller.set_text(4, DisplayLine::Upper, "BACK");
        let start = start + Duration::from_secs(1);
        assert_eq!(
            drain(&mut controller, start, start + Duration::from_secs(1)).len(),
            0
        );
        let start = start + Duration::from_secs(1);
        controller.connected(start);
        let sent = drain(&mut controller, start, start + Duration::from_secs(1));
        assert_eq!(sent.len(), 156);
        assert_eq!(controller.desired().fader(Fader::Strip(4)), Some(u16::MAX));
        assert_eq!(controller.desired(), controller.shadow());
    }

    #[test]
    fn the_curves_can_be_replaced_without_touching_the_translation() {
        // They are taste rather than measurement, so a settings screen owns them.
        let (mut controller, start) = settled();
        let (vpot, jog) = controller.curves();
        assert_eq!(vpot.steps(8), 36);
        controller.set_curves(VPotAcceleration { curve: &[2] }, jog);
        let mut events = Vec::new();
        controller.push(&[0xB0, 16, 0x03], start, |event| events.push(event));
        assert_eq!(events, vec![SurfaceEvent::Encoder { strip: 0, steps: 2 }]);
    }

    #[test]
    fn a_fader_index_the_arrays_do_not_have_reaches_no_state() {
        // `fader_at` is the inverse the rebuild loop walks; a controller that
        // trusted it further than the arrays go would index past the end, which
        // the crate denies outright.
        assert_eq!(fader_at(9), None);
        let controller = SurfaceController::new(X_TOUCH);
        assert!(!controller.is_touched(Fader::Strip(9)));
        assert_eq!(controller.timing(), SurfaceTiming::default());
        assert_eq!(controller.profile().name, X_TOUCH.name);
        assert_eq!(controller.mode(), SurfaceMode::Dedicated);
    }

    #[test]
    fn the_bit_helpers_are_total_so_a_wider_surface_cannot_corrupt_them() {
        // The shadow model indexes sixty-four panel LEDs into a `u64`, which is
        // exactly full. A profile with a sixty-fifth button would otherwise
        // shift past the end - a panic, on a crate that denies panicking.
        assert!(!bit(u64::MAX, 64));
        assert_eq!(with_bit(0, 64), 0);
        assert_eq!(without_bit(u64::MAX, 64), u64::MAX);
        assert!(bit(with_bit(0, 63), 63));
        assert!(!bit(without_bit(u64::MAX, 63), 63));
    }

    #[test]
    fn a_surface_that_speaks_after_it_has_gone_does_not_come_back_by_itself() {
        // Bytes can still be in a buffer when the port is declared gone.
        // Reviving the health from them would light a green lamp for a desk the
        // caller has already given up on; only `connected` starts it again.
        let (mut controller, start) = settled();
        controller.disconnected();
        let mut events = Vec::new();
        controller.push(&[0x90, 94, 0x7F], start, |event| events.push(event));
        assert_eq!(events.len(), 1, "the press is still worth having");
        assert_eq!(controller.health(), SurfaceHealth::Disconnected);
    }

    #[test]
    fn an_overload_marker_is_a_meter_signal_and_not_a_level() {
        // The two codes above the levels set and clear the marker, so they pass
        // through the clamp untouched.
        let (mut controller, start) = settled();
        controller.set_meter(2, MeterSignal::Overload(true));
        assert_eq!(
            controller.desired().meter(2),
            Some(MeterSignal::Overload(true))
        );
        let sent = drain(&mut controller, start, start + Duration::from_millis(200));
        assert_eq!(
            sent.first().map(|message| message.bytes.as_slice()),
            Some(&[0xD0, 0x2E][..])
        );
    }

    #[test]
    fn a_character_no_display_can_draw_becomes_one_a_person_can_see() {
        // Two ways to be undrawable: outside ASCII altogether, and inside it but
        // not printable. A tab in an executor name must not become a hole in a
        // scribble strip that only the desk knows about.
        assert_eq!(display_byte('\t'), b'?');
        assert_eq!(display_byte('\u{007F}'), b'?');
        assert_eq!(display_byte('\u{00DC}'), b'?');
        assert_eq!(display_byte('a'), b'A');
        assert_eq!(display_byte(' '), b' ');
    }
}
