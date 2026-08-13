//! Layer 2's vocabulary: the controls, the picture on them, and who owns them.
//!
//! `docs/MCU_MAPPING.md` §3 lists the logical control set this layer works in —
//! `Strip[n].Fader`, `Strip[n].Button.Select`, `Global.Play` — and says the
//! layer owns *the shadow model: the last state believed to be displayed on the
//! device*. That is [`SurfaceState`], and there are two of them in
//! [`SurfaceController`](crate::SurfaceController): what the show wants shown,
//! and what the desk was last told.
//!
//! # What this layer adds to layer 1
//!
//! Layer 1's [`ControlEvent`](crate::ControlEvent) is already device-independent
//! in shape; what it is not is *interpreted*. This layer turns a raw fader
//! position into a level, a reported detent count into parameter steps, and an
//! arbitrary colour into one of the eight a scribble strip has — three
//! conversions that are wrong in three different ways if a caller does them by
//! eye:
//!
//! - a fader divided by [`FADER_MAX`](crate::FADER_MAX) rather than by what the
//!   surface actually reports gives 99.98 % at the end stop (§2.7);
//! - one acceleration curve for both relative controls is wrong about one of
//!   them (`accel`);
//! - a colour matched by distance rather than by hue turns a pastel white
//!   (`color`).
//!
//! # Sizes are constants here and data on the profile
//!
//! The arrays below are sized for the surface this crate describes. A test holds
//! them against [`X_TOUCH`](crate::X_TOUCH), so a profile that outgrew them
//! fails loudly rather than silently dropping the ninth strip.

use crate::control::ButtonId;
use crate::feedback::{
    Feedback, LedState, MeterSignal, RingMode, SCRIBBLE_STRIP_COLORS, SegmentChar, StripColor,
    VPotRing,
};
use crate::profile::{Fader, GlobalButton, StripButton};

/// Channel strips the state arrays are sized for.
pub const MAX_STRIPS: usize = 8;

/// Faders the state arrays are sized for: one per strip plus the main fader.
pub const MAX_FADERS: usize = MAX_STRIPS + 1;

/// 7-segment digits the state arrays are sized for.
pub const MAX_SEGMENTS: usize = 12;

/// Characters one strip owns on one scribble strip line.
pub const STRIP_CHARS: usize = 7;

/// Lines a scribble strip has.
pub const DISPLAY_LINES: usize = 2;

/// Buttons a channel strip has.
pub const STRIP_BUTTONS: usize = StripButton::ALL.len();

/// Buttons the panel has that do not belong to a strip.
pub const GLOBAL_BUTTONS: usize = GlobalButton::ALL.len();

/// The character a blank 7-segment digit holds.
///
/// 32 rather than 0: both blank the digit (§2.7), and 32 is the one the
/// stripping rule produces for a space, so a shadow holding it does not flicker
/// into the other when a caller writes a space.
pub const SEGMENT_BLANK: u8 = b' ' & 0x3F;

/// How much of the surface is PrismDMX's.
///
/// **This is the operator's account of the desk rather than a measurement** —
/// `docs/MCU_MAPPING.md` §4.3 marks it as such and §7 carries it as an open
/// item. It is modelled anyway, because feedback that assumes it owns a control
/// it does not own is feedback that fights another host's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SurfaceMode {
    /// The whole surface is PrismDMX's — a desk in plain MC mode, and what the
    /// operator gets after pressing SMPTE/Beats in the combined mode.
    #[default]
    Dedicated,
    /// The surface is shared with a sound console over Xctl, and only what Xctl
    /// leaves unused reaches MC: in practice the transport section and the jog
    /// wheel ([`McuProfile::permanent`](crate::McuProfile::permanent)).
    ///
    /// Nothing is unreachable in this mode — the whole surface is one button
    /// away — but until that button is pressed the strips belong to the sound
    /// desk, and lighting a Select LED there is at best ignored.
    Shared,
}

/// One logical control, named the way `docs/MCU_MAPPING.md` §3 names it.
///
/// A control rather than a message: `Control::Encoder(3)` is strip 3's V-Pot,
/// which is a turn on the way in and a ring of LEDs on the way out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Control {
    /// A motor fader and its touch sensor.
    Fader(Fader),
    /// A button and its LED.
    Button(ButtonId),
    /// A strip's V-Pot: the encoder, its push, and its ring of LEDs.
    Encoder(u8),
    /// A strip's scribble strip: two lines of text and a backlight colour.
    Display(u8),
    /// A strip's level meter.
    Meter(u8),
    /// One digit of the 7-segment display, counted from the right.
    Segment(u8),
    /// The jog wheel.
    Jog,
}

/// Which line of a scribble strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DisplayLine {
    /// The upper line. `docs/MCU_MAPPING.md` §4.1 gives it the executor's name.
    Upper,
    /// The lower line, which the same row gives its value.
    Lower,
}

impl DisplayLine {
    /// Both lines, so a test walks them rather than the one somebody remembered.
    pub const ALL: [Self; DISPLAY_LINES] = [Self::Upper, Self::Lower];

    /// This line's index into the display arrays.
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Upper => 0,
            Self::Lower => 1,
        }
    }
}

/// Something the operator did, interpreted.
///
/// [`Copy`] and free of owned fields, exactly as
/// [`ControlEvent`](crate::ControlEvent) is: this is the value that crosses the
/// thread boundary onto the command bus, on the path `ARCHITECTURE_SPEC.md` §4.3
/// budgets in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceEvent {
    /// A button went down or came up.
    Button {
        /// Which button.
        button: ButtonId,
        /// `true` for a press.
        pressed: bool,
    },
    /// A hand arrived on a fader or left it.
    Touch {
        /// Which fader.
        fader: Fader,
        /// `true` while a hand is on it.
        touched: bool,
    },
    /// A fader moved, as a level rather than as a wire position.
    Moved {
        /// Which fader.
        fader: Fader,
        /// 0…65535, scaled against what this surface actually reports at the
        /// top of travel — see
        /// [`McuProfile::level_from_position`](crate::McuProfile::level_from_position).
        level: u16,
    },
    /// A V-Pot was turned, through its acceleration curve.
    Encoder {
        /// Strip index, 0 at the left.
        strip: u8,
        /// Parameter steps, positive clockwise.
        steps: i32,
    },
    /// The jog wheel was turned, through *its* acceleration curve, which is a
    /// different one — see the `accel` module.
    Jog {
        /// Parameter steps, positive clockwise.
        steps: i32,
    },
}

/// The order outbound messages are sent in when there is not room for all of
/// them, from `docs/MCU_MAPPING.md` §5.2.
///
/// The declaration order is the send order, and [`Ord`] is derived from it, so
/// sorting a batch of messages by priority is the rule rather than an
/// implementation of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    /// Motor faders — most visible, most misleading if stale.
    Fader,
    /// Button and ring LEDs — state feedback the operator acts on.
    Led,
    /// Scribble strips and the 7-segment display — tolerant of a late update.
    Display,
    /// Level meters — decorative, and **dropped first**. A dropped meter falls,
    /// it does not freeze: the surface decays it in well under a second (§2.7).
    Meter,
}

impl Priority {
    /// Every class, highest first.
    pub const ALL: [Self; 4] = [Self::Fader, Self::Led, Self::Display, Self::Meter];

    /// The class a message belongs to, or `None` for one that is not part of
    /// the picture on the surface — the handshake, and the LCD meter mode this
    /// device ignores anyway (§2.7).
    #[must_use]
    pub const fn of(feedback: &Feedback<'_>) -> Option<Self> {
        match *feedback {
            Feedback::Move { .. } => Some(Self::Fader),
            Feedback::Led { .. } | Feedback::Ring { .. } => Some(Self::Led),
            Feedback::DisplayText { .. }
            | Feedback::DisplayColors(_)
            | Feedback::Segment { .. } => Some(Self::Display),
            Feedback::Meter { .. } => Some(Self::Meter),
            Feedback::MeterMode(_) | Feedback::DeviceQuery => None,
        }
    }
}

/// One scribble strip: two lines of text and a backlight colour, held apart.
///
/// **Text and colour are separate state on the device** — S20 wrote eight cyan
/// strips and then wrote text over them, and the colour survived (§2.7). So they
/// diff separately here, and a name changing does not cost the colour message as
/// well.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StripDisplay {
    /// The two lines, [`STRIP_CHARS`] characters each, space-padded.
    pub lines: [[u8; STRIP_CHARS]; DISPLAY_LINES],
}

impl StripDisplay {
    /// A blank strip.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            lines: [[b' '; STRIP_CHARS]; DISPLAY_LINES],
        }
    }

    /// One line's characters.
    #[must_use]
    pub fn line(&self, line: DisplayLine) -> &[u8; STRIP_CHARS] {
        self.lines.get(line.index()).unwrap_or(&[b' '; STRIP_CHARS])
    }
}

impl Default for StripDisplay {
    fn default() -> Self {
        Self::new()
    }
}

/// A picture of the whole surface.
///
/// Two of these make the shadow model: one holding what the show wants shown,
/// one holding what the desk was last told. Outbound traffic is the difference
/// between them, which is what `docs/MCU_MAPPING.md` §3 means by *never by
/// re-sending everything*.
///
/// Fixed-size and free of any owned buffer, so a controller holding two of them
/// allocates nothing, ever.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceState {
    /// Fader levels, 0…65535, indexed by [`fader_index`].
    pub(crate) faders: [u16; MAX_FADERS],
    /// Strip button LEDs, indexed by strip and then by [`StripButton::index`].
    pub(crate) strip_leds: [[LedState; STRIP_BUTTONS]; MAX_STRIPS],
    /// Panel button LEDs, indexed by [`GlobalButton::index`].
    pub(crate) global_leds: [LedState; GLOBAL_BUTTONS],
    /// V-Pot rings.
    pub(crate) rings: [VPotRing; MAX_STRIPS],
    /// Scribble strip text.
    pub(crate) displays: [StripDisplay; MAX_STRIPS],
    /// Scribble strip colours. One array rather than one per strip, because the
    /// message is **all eight or nothing** — any other length is ignored by the
    /// device (§2.7).
    pub(crate) colors: [StripColor; SCRIBBLE_STRIP_COLORS],
    /// Level meters.
    pub(crate) meters: [MeterSignal; MAX_STRIPS],
    /// The 7-segment display, digit 0 rightmost.
    pub(crate) segments: [SegmentChar; MAX_SEGMENTS],
}

impl SurfaceState {
    /// A surface showing nothing: faders down, LEDs dark, strips blank.
    ///
    /// The colours start **white** rather than off, which is the one
    /// non-obvious default and comes straight from §2.3: black is the backlight
    /// *off* and its text cannot be read, so an unassigned strip wants white and
    /// black is reserved for a deliberate *nothing here*.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            faders: [0; MAX_FADERS],
            strip_leds: [[LedState::Off; STRIP_BUTTONS]; MAX_STRIPS],
            global_leds: [LedState::Off; GLOBAL_BUTTONS],
            rings: [VPotRing {
                mode: RingMode::Dot,
                position: 0,
                led: false,
            }; MAX_STRIPS],
            displays: [StripDisplay::new(); MAX_STRIPS],
            colors: [StripColor::White; SCRIBBLE_STRIP_COLORS],
            meters: [MeterSignal::Level(0); MAX_STRIPS],
            segments: [SegmentChar {
                code: SEGMENT_BLANK,
                dot: false,
            }; MAX_SEGMENTS],
        }
    }

    /// A fader's level, 0…65535.
    #[must_use]
    pub fn fader(&self, fader: Fader) -> Option<u16> {
        self.faders.get(fader_index(fader)?).copied()
    }

    /// A button's LED.
    #[must_use]
    pub fn led(&self, button: ButtonId) -> Option<LedState> {
        match button {
            ButtonId::Strip { strip, button } => self
                .strip_leds
                .get(usize::from(strip))?
                .get(button.index())
                .copied(),
            ButtonId::Global(button) => self.global_leds.get(button.index()).copied(),
        }
    }

    /// A V-Pot's ring.
    #[must_use]
    pub fn ring(&self, strip: u8) -> Option<VPotRing> {
        self.rings.get(usize::from(strip)).copied()
    }

    /// One line of one scribble strip.
    #[must_use]
    pub fn text(&self, strip: u8, line: DisplayLine) -> Option<&[u8; STRIP_CHARS]> {
        Some(self.displays.get(usize::from(strip))?.line(line))
    }

    /// A scribble strip's backlight colour.
    #[must_use]
    pub fn color(&self, strip: u8) -> Option<StripColor> {
        self.colors.get(usize::from(strip)).copied()
    }

    /// A strip's meter.
    #[must_use]
    pub fn meter(&self, strip: u8) -> Option<MeterSignal> {
        self.meters.get(usize::from(strip)).copied()
    }

    /// One 7-segment digit, counted from the right.
    #[must_use]
    pub fn segment(&self, digit: u8) -> Option<SegmentChar> {
        self.segments.get(usize::from(digit)).copied()
    }
}

impl Default for SurfaceState {
    fn default() -> Self {
        Self::new()
    }
}

/// Every fader, in the order the state arrays hold them.
///
/// The rebuild loop walks this rather than counting to [`MAX_FADERS`] and
/// converting: a loop over indices needs an arm for an index that cannot
/// happen, and an arm that cannot happen is one no test can reach.
pub const ALL_FADERS: [Fader; MAX_FADERS] = [
    Fader::Strip(0),
    Fader::Strip(1),
    Fader::Strip(2),
    Fader::Strip(3),
    Fader::Strip(4),
    Fader::Strip(5),
    Fader::Strip(6),
    Fader::Strip(7),
    Fader::Main,
];

/// A fader's index into the state arrays: the strips in order, then the main
/// fader.
///
/// `None` for a strip this crate's arrays are not sized for, which is a paging
/// fault upstairs rather than something to wrap around.
#[must_use]
pub const fn fader_index(fader: Fader) -> Option<usize> {
    match fader {
        Fader::Strip(strip) if (strip as usize) < MAX_STRIPS => Some(strip as usize),
        Fader::Strip(_) => None,
        Fader::Main => Some(MAX_STRIPS),
    }
}

/// The fader at an index, the inverse of [`fader_index`].
#[must_use]
pub const fn fader_at(index: usize) -> Option<Fader> {
    if index < MAX_STRIPS {
        Some(Fader::Strip(index as u8))
    } else if index == MAX_STRIPS {
        Some(Fader::Main)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Control, DISPLAY_LINES, DisplayLine, GLOBAL_BUTTONS, MAX_FADERS, MAX_SEGMENTS, MAX_STRIPS,
        Priority, SEGMENT_BLANK, STRIP_BUTTONS, STRIP_CHARS, SurfaceMode, SurfaceState, fader_at,
        fader_index,
    };
    use crate::control::ButtonId;
    use crate::feedback::{
        Feedback, LcdMeterMode, LedState, MeterSignal, RingMode, SCRIBBLE_STRIP_COLORS,
        SegmentChar, StripColor, VPotRing,
    };
    use crate::profile::{Fader, GlobalButton, StripButton, X_TOUCH};

    #[test]
    fn the_arrays_are_sized_for_the_surface_the_profile_describes() {
        // The one place layer 2 holds a number layer 1 holds as data. A profile
        // that outgrew these arrays would otherwise lose its ninth strip in
        // silence, which is the failure mode `McuProfile` exists to avoid.
        assert_eq!(usize::from(X_TOUCH.strips), MAX_STRIPS);
        assert_eq!(usize::from(X_TOUCH.segments), MAX_SEGMENTS);
        assert_eq!(usize::from(X_TOUCH.lcd_chars_per_strip), STRIP_CHARS);
        assert_eq!(MAX_FADERS, MAX_STRIPS + 1);
        assert_eq!(SCRIBBLE_STRIP_COLORS, MAX_STRIPS);
        assert_eq!(STRIP_BUTTONS, 5);
        assert_eq!(GLOBAL_BUTTONS, 64);
        assert_eq!(
            usize::from(X_TOUCH.lcd_buffer_len),
            MAX_STRIPS * STRIP_CHARS * DISPLAY_LINES
        );
    }

    #[test]
    fn a_fader_index_is_a_bijection_and_the_main_fader_is_not_strip_eight() {
        for index in 0..MAX_FADERS {
            let fader = fader_at(index).expect("an index the arrays have");
            assert_eq!(fader_index(fader), Some(index));
        }
        assert_eq!(fader_index(Fader::Main), Some(MAX_STRIPS));
        assert_eq!(fader_index(Fader::Strip(8)), None);
        assert_eq!(fader_at(MAX_FADERS), None);
    }

    #[test]
    fn the_blank_state_is_readable_rather_than_dark() {
        // §2.3: a strip set to black has its backlight off and its text cannot
        // be read. So the state a desk is put into before anything is assigned
        // is white and blank, not black.
        let state = SurfaceState::new();
        for strip in 0..8u8 {
            assert_eq!(state.color(strip), Some(StripColor::White));
            for line in DisplayLine::ALL {
                assert_eq!(state.text(strip, line), Some(&[b' '; STRIP_CHARS]));
            }
            assert_eq!(state.meter(strip), Some(MeterSignal::Level(0)));
            assert_eq!(
                state.ring(strip),
                Some(VPotRing {
                    mode: RingMode::Dot,
                    position: 0,
                    led: false
                })
            );
        }
        for digit in 0..12u8 {
            assert_eq!(
                state.segment(digit),
                Some(SegmentChar {
                    code: SEGMENT_BLANK,
                    dot: false
                })
            );
        }
        assert_eq!(state.fader(Fader::Main), Some(0));
        assert_eq!(
            state.led(ButtonId::Global(GlobalButton::Play)),
            Some(LedState::Off)
        );
        assert_eq!(SurfaceState::default(), state);
    }

    #[test]
    fn asking_the_state_about_a_control_it_has_not_got_answers_nothing() {
        // Every accessor is an `Option` rather than an index, because the crate
        // denies slice indexing outside its tests and because a ninth strip is
        // a question a paging bug asks.
        let state = SurfaceState::new();
        assert_eq!(state.fader(Fader::Strip(9)), None);
        assert_eq!(state.ring(8), None);
        assert_eq!(state.color(8), None);
        assert_eq!(state.meter(8), None);
        assert_eq!(state.segment(12), None);
        assert_eq!(state.text(8, DisplayLine::Upper), None);
        assert_eq!(
            state.led(ButtonId::Strip {
                strip: 8,
                button: StripButton::Rec
            }),
            None
        );
    }

    #[test]
    fn the_send_order_is_the_one_the_document_gives() {
        // §5.2, as an ordering rather than as a comment: faders, LEDs, scribble
        // strips, meters. Derived `Ord` means sorting is the rule itself.
        assert!(Priority::Fader < Priority::Led);
        assert!(Priority::Led < Priority::Display);
        assert!(Priority::Display < Priority::Meter);
        let mut sorted = Priority::ALL;
        sorted.sort_unstable();
        assert_eq!(sorted, Priority::ALL);
    }

    #[test]
    fn every_message_is_classified_and_the_two_that_are_not_state_say_so() {
        let ring = VPotRing {
            mode: RingMode::Dot,
            position: 1,
            led: false,
        };
        let character = SegmentChar::from_ascii(b'7').expect("a digit");
        for (message, want) in [
            (
                Feedback::Move {
                    fader: Fader::Main,
                    position: 0,
                },
                Some(Priority::Fader),
            ),
            (
                Feedback::Led {
                    button: ButtonId::Global(GlobalButton::Play),
                    state: LedState::On,
                },
                Some(Priority::Led),
            ),
            (Feedback::Ring { strip: 0, ring }, Some(Priority::Led)),
            (
                Feedback::DisplayText {
                    offset: 0,
                    text: b"X",
                },
                Some(Priority::Display),
            ),
            (
                Feedback::DisplayColors([StripColor::White; SCRIBBLE_STRIP_COLORS]),
                Some(Priority::Display),
            ),
            (
                Feedback::Segment {
                    digit: 0,
                    character,
                },
                Some(Priority::Display),
            ),
            (
                Feedback::Meter {
                    strip: 0,
                    signal: MeterSignal::Level(3),
                },
                Some(Priority::Meter),
            ),
            (Feedback::MeterMode(LcdMeterMode::Horizontal), None),
            (Feedback::DeviceQuery, None),
        ] {
            assert_eq!(Priority::of(&message), want, "{message:?}");
        }
    }

    #[test]
    fn a_control_names_itself_once_and_compares_by_value() {
        // The ownership set on the profile is a list of these, so two ways of
        // writing the same control would make `permanent` silently not contain
        // what it says it contains.
        assert_eq!(
            Control::Button(ButtonId::Global(GlobalButton::Play)),
            Control::Button(ButtonId::Global(GlobalButton::Play))
        );
        assert_ne!(Control::Encoder(0), Control::Meter(0));
        assert_ne!(Control::Fader(Fader::Main), Control::Fader(Fader::Strip(0)));
    }

    #[test]
    fn a_dedicated_desk_is_the_default_because_a_shared_one_is_a_deployment() {
        assert_eq!(SurfaceMode::default(), SurfaceMode::Dedicated);
        assert_eq!(DisplayLine::Upper.index(), 0);
        assert_eq!(DisplayLine::Lower.index(), 1);
    }

    #[test]
    fn every_fader_is_in_the_list_the_diff_walks() {
        // `ALL_FADERS` and `fader_index` are two statements of one fact, and the
        // rebuild loop trusts both: it enumerates the first and indexes the
        // arrays with the position.
        for (index, fader) in super::ALL_FADERS.into_iter().enumerate() {
            assert_eq!(fader_index(fader), Some(index), "{fader}");
            assert_eq!(fader_at(index), Some(fader));
        }
        assert_eq!(super::ALL_FADERS.len(), MAX_FADERS);
        assert_eq!(super::ALL_FADERS.last(), Some(&Fader::Main));
    }

    #[test]
    fn a_strip_button_led_is_read_out_of_the_state_by_strip_and_button() {
        // The success path of the two-dimensional lookup, which the
        // out-of-range test above cannot reach.
        let state = SurfaceState::new();
        for strip in 0..8u8 {
            for button in StripButton::ALL {
                assert_eq!(
                    state.led(ButtonId::Strip { strip, button }),
                    Some(LedState::Off),
                    "strip {strip} {button}"
                );
            }
        }
        assert_eq!(super::StripDisplay::default(), super::StripDisplay::new());
    }
}
