//! Executors — the eight faders of the current page (D7).
//!
//! Each executor binds a sequence to a fader, four buttons and an encoder. The
//! function assignments are data, not code paths, because the same executor
//! model is driven from the X-Touch and from the UI.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{ExecutorId, SequenceId};

/// Executors per page (decision **D7**).
pub const EXECUTORS_PER_PAGE: u32 = 8;

/// [`Executor::speed`] at which a sequence plays at the times its cues carry.
///
/// A fixed-point rate rather than a float: it crosses into the tick, where
/// `prism_engine` multiplies it into an elapsed-tick accumulator, and a power of
/// two keeps that exact. `0` freezes a playback, [`SPEED_UNITY`] is 1x, and
/// `u16::MAX` is just under 64x — which is the range a *speed master* covers
/// (`docs/DMX_MERGE.md` section 4 item 3), the thing S34 built and S22 recorded
/// as having no domain type at all.
pub const SPEED_UNITY: u16 = 1_024;

/// What one of an executor's four buttons does.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
pub enum ExecutorButtonFunction {
    /// Nothing assigned.
    #[default]
    Empty,
    /// Advance to the next cue.
    #[serde(rename = "Go+")]
    GoForward,
    /// Step back to the previous cue.
    #[serde(rename = "Go-")]
    GoBack,
    /// Learn the sequence speed from the tap rhythm.
    LearnSpeed,
    /// Stop the sequence.
    Off,
    /// Start the sequence.
    On,
    /// Momentary full level while held.
    Flash,
    /// Latch on and off.
    Toggle,
}

/// Which of an executor's buttons a press names.
///
/// **The executor decides what a press means, not whoever sent it.** A strip
/// button is a *position* on the hardware — the operator pressed the third key
/// of that strip — and what that key does is [`Executor::button_functions`],
/// which is show data. A client that read `is_active` and sent a Go or an Off
/// because the show said `Toggle` would be deciding for the show: two clients
/// would race, and the daemon would be told to do something nobody pressed.
/// `prism_core::Show::apply` is the one place that resolution happens.
///
/// [`Self::Function`] is the other half of the same rule rather than a hole in
/// it. `docs/MCU_MAPPING.md` section 4.1 binds the transport keys to `On`, `Off`,
/// `Go+` and `Go-` **in the profile**, which is the desk's own configuration
/// written by a person, not an inference a client made at run time. A binding
/// that names `Toggle` is still resolved by the daemon against `is_active`; what
/// the profile chose is *which function*, never *what it comes out as*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(tag = "t", rename_all_fields = "camelCase")]
pub enum ExecutorButtonRef {
    /// The executor's own button at a hardware position: Rec, Solo, Mute,
    /// Select = 0..4 (`docs/MCU_MAPPING.md` section 2.1).
    ///
    /// A position the executor has no function for is not an error — it is a key
    /// with nothing on it, and pressing it does nothing.
    Slot {
        /// Which of the four, from zero.
        index: u8,
    },
    /// A function bound directly by the desk's configuration — a profile row, a
    /// transport key, an interface's own key.
    Function {
        /// What was bound.
        function: ExecutorButtonFunction,
    },
}

/// What an executor's fader does.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum ExecutorFaderFunction {
    /// Nothing assigned.
    #[default]
    Empty,
    /// Master level of the sequence.
    Master,
    /// Playback speed.
    Speed,
    /// Manual crossfade between cues.
    XFade,
}

/// What an executor's encoder does.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum ExecutorEncoderFunction {
    /// Nothing assigned.
    #[default]
    Empty,
    /// Master level of the sequence.
    Master,
    /// Playback speed.
    Speed,
}

/// One executor: a sequence bound to a fader, four buttons and an encoder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct Executor {
    /// Executor number: `page * 8 + slot`.
    pub id: ExecutorId,
    /// The sequence this executor plays, if one is assigned.
    pub sequence_id: Option<SequenceId>,
    /// Fader assignment.
    pub fader_function: ExecutorFaderFunction,
    /// Button assignments, in hardware order: Rec, Solo, Mute, Select.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(4)")
    )]
    pub button_functions: Vec<ExecutorButtonFunction>,
    /// Encoder assignment.
    pub encoder_function: ExecutorEncoderFunction,
    /// Current master level, `0..=65535`.
    pub master_level: u16,
    /// Playback rate, in units of [`SPEED_UNITY`].
    ///
    /// The *speed master* of `docs/DMX_MERGE.md` section 4 item 3: it changes how
    /// fast the cue list runs, never what any attribute resolves to. An
    /// executor whose fader or encoder is set to `Speed` moves this rather than
    /// [`Self::master_level`], and `ExecutorButtonFunction::LearnSpeed` taps it
    /// in.
    ///
    /// **Defaulted on the way in, always written on the way out.** A `.prism`
    /// file keeps each executor as an opaque document (S15), so a field added
    /// later is a field older files do not carry — and an executor written
    /// before S34 was playing at unity, which is exactly what the default says.
    /// A schema migration cannot help here: the migration mechanism rewrites
    /// *tables*, and this lives inside a MessagePack blob.
    #[serde(default = "unity_speed")]
    #[cfg_attr(any(test, feature = "proptest"), proptest(strategy = "0..=u16::MAX"))]
    pub speed: u16,
    /// Whether the sequence is currently running.
    ///
    /// **Written only by the tick's readback** (S34) —
    /// `prism_engine::PlaybackReport` through `prismd::Core::poll_playback`. A
    /// command that starts a playback does not set it on the way past: the
    /// command has only been *queued* when it is acknowledged, and two authors
    /// for one field means the loser is whichever arrives second.
    pub is_active: bool,
    /// Index of the current cue within the sequence, if one is active.
    ///
    /// The same, and it is the half nothing could know before S34: what cue a
    /// playback is on lives on the tick thread.
    pub current_cue_index: Option<u32>,
}

/// The rate an executor written before [`Executor::speed`] existed was playing
/// at, which is the only rate it could have been playing at.
const fn unity_speed() -> u16 {
    SPEED_UNITY
}

impl ExecutorId {
    /// The executor page this executor belongs to.
    #[must_use]
    pub const fn page(self) -> u32 {
        self.get() / EXECUTORS_PER_PAGE
    }

    /// The slot within its page, `0..8`.
    #[must_use]
    pub const fn slot(self) -> u32 {
        self.get() % EXECUTORS_PER_PAGE
    }

    /// Builds an executor number from a page and a slot.
    ///
    /// Saturates rather than wrapping. `SetExecutorPage` carries an arbitrary
    /// `u32` from any client, and in a release build `page * 8` would wrap
    /// silently for a large one — addressing the wrong executor during a show.
    /// Saturating keeps an out-of-range page out of range instead.
    #[must_use]
    pub const fn from_page_and_slot(page: u32, slot: u32) -> Self {
        Self::new(page.saturating_mul(EXECUTORS_PER_PAGE).saturating_add(slot))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Executor, ExecutorButtonFunction, ExecutorButtonRef, ExecutorEncoderFunction,
        ExecutorFaderFunction, ExecutorId, SequenceId,
    };
    use ts_rs::{Config, TS};

    fn executor() -> Executor {
        Executor {
            id: ExecutorId::new(9),
            sequence_id: Some(SequenceId::new(1)),
            fader_function: ExecutorFaderFunction::Master,
            button_functions: vec![
                ExecutorButtonFunction::GoForward,
                ExecutorButtonFunction::GoBack,
            ],
            encoder_function: ExecutorEncoderFunction::Speed,
            master_level: 65535,
            speed: crate::SPEED_UNITY,
            is_active: true,
            current_cue_index: Some(0),
        }
    }

    #[test]
    fn executor_matches_the_wire_shape() {
        assert_eq!(
            serde_json::to_string(&executor()).unwrap(),
            r#"{"id":9,"sequenceId":1,"faderFunction":"Master","buttonFunctions":["Go+","Go-"],"encoderFunction":"Speed","masterLevel":65535,"speed":1024,"isActive":true,"currentCueIndex":0}"#
        );
    }

    #[test]
    fn go_buttons_keep_their_console_labels() {
        assert_eq!(
            serde_json::to_string(&ExecutorButtonFunction::GoForward).unwrap(),
            "\"Go+\""
        );
        assert_eq!(
            serde_json::to_string(&ExecutorButtonFunction::GoBack).unwrap(),
            "\"Go-\""
        );
        let back: ExecutorButtonFunction = serde_json::from_str("\"Go-\"").unwrap();
        assert_eq!(back, ExecutorButtonFunction::GoBack);
    }

    #[test]
    fn an_unassigned_slot_is_empty_not_absent() {
        assert_eq!(
            ExecutorFaderFunction::default(),
            ExecutorFaderFunction::Empty
        );
        assert_eq!(
            ExecutorEncoderFunction::default(),
            ExecutorEncoderFunction::Empty
        );
        assert_eq!(
            ExecutorButtonFunction::default(),
            ExecutorButtonFunction::Empty
        );
    }

    #[test]
    fn executor_ids_decompose_into_page_and_slot() {
        // D7: one page is eight executors, so id = page * 8 + slot.
        assert_eq!(ExecutorId::new(0).page(), 0);
        assert_eq!(ExecutorId::new(0).slot(), 0);
        assert_eq!(ExecutorId::new(9).page(), 1);
        assert_eq!(ExecutorId::new(9).slot(), 1);
        assert_eq!(ExecutorId::from_page_and_slot(1, 1), ExecutorId::new(9));
    }

    #[test]
    fn an_absurd_page_number_saturates_instead_of_wrapping() {
        // SetExecutorPage carries an arbitrary u32 from any client. Wrapping
        // would silently address a low executor - a live show hazard.
        assert_eq!(
            ExecutorId::from_page_and_slot(u32::MAX, 7),
            ExecutorId::new(u32::MAX)
        );
        assert_eq!(
            ExecutorId::from_page_and_slot(u32::MAX / 8, 0).get(),
            (u32::MAX / 8) * 8
        );
    }

    #[test]
    fn a_button_press_names_a_position_or_a_function_and_the_two_are_distinguishable() {
        // A strip press is a *position*: what it does is the executor's own
        // `buttonFunctions`, resolved by `prism_core::Show::apply` and nowhere
        // else. A profile row names a function outright, which is the desk's
        // configuration rather than a client's inference. The wire has to keep
        // the two apart, so the tag is not optional.
        assert_eq!(
            serde_json::to_string(&ExecutorButtonRef::Slot { index: 2 }).unwrap(),
            r#"{"t":"Slot","index":2}"#
        );
        assert_eq!(
            serde_json::to_string(&ExecutorButtonRef::Function {
                function: ExecutorButtonFunction::Toggle
            })
            .unwrap(),
            r#"{"t":"Function","function":"Toggle"}"#
        );
        for reference in [
            ExecutorButtonRef::Slot { index: 0 },
            ExecutorButtonRef::Slot { index: u8::MAX },
            ExecutorButtonRef::Function {
                function: ExecutorButtonFunction::GoForward,
            },
        ] {
            let json = serde_json::to_string(&reference).unwrap();
            let back: ExecutorButtonRef = serde_json::from_str(&json).unwrap();
            assert_eq!(back, reference);
        }
    }

    #[test]
    fn an_executor_written_before_speed_existed_reads_as_unity() {
        // A `.prism` file keeps each executor as an opaque document (S15), so a
        // field added in S34 is a field every older file is missing — and the
        // migration mechanism rewrites *tables*, not the MessagePack inside
        // them. The default is the only rate such an executor could have been
        // playing at. `prism_core`'s frozen version-1 fixture is the other end
        // of this claim; this is the end that says what the rule is.
        let json = r#"{"id":9,"sequenceId":null,"faderFunction":"Master",
            "buttonFunctions":[],"encoderFunction":"Empty","masterLevel":40000,
            "isActive":false,"currentCueIndex":null}"#;
        let executor: Executor = serde_json::from_str(json).expect("an older document still reads");
        assert_eq!(executor.speed, crate::SPEED_UNITY);
        assert_eq!(executor.master_level, 40_000);
        // And it is written back out with the field, always: the default is on
        // the way in only.
        assert!(
            serde_json::to_string(&executor)
                .unwrap()
                .contains(r#""speed":1024"#)
        );
    }

    #[test]
    fn speed_unity_is_the_rate_a_sequence_plays_its_own_times_at() {
        // A power of two, because the tick multiplies it into an accumulator and
        // divides by it again: anything else quantises a fade differently at
        // unity than at rest, which is a fade that changes shape when nobody
        // touched it.
        assert_eq!(crate::SPEED_UNITY, 1_024);
        assert!(crate::SPEED_UNITY.is_power_of_two());
        // The whole speed range in one line: frozen, unity, and just under 64x.
        assert_eq!(u32::from(u16::MAX) / u32::from(crate::SPEED_UNITY), 63);
    }

    #[test]
    fn typescript_unions_match_the_specification() {
        let cfg = Config::new();
        assert_eq!(
            ExecutorButtonFunction::inline(&cfg),
            "\"Empty\" | \"Go+\" | \"Go-\" | \"LearnSpeed\" | \"Off\" | \"On\" | \"Flash\" \
             | \"Toggle\""
        );
        assert_eq!(
            ExecutorFaderFunction::inline(&cfg),
            "\"Empty\" | \"Master\" | \"Speed\" | \"XFade\""
        );
        assert_eq!(
            ExecutorEncoderFunction::inline(&cfg),
            "\"Empty\" | \"Master\" | \"Speed\""
        );
    }
}

impl ExecutorButtonFunction {
    /// Every function a key can be given, so a test — and the control editor's
    /// chooser — walks the whole set rather than the ones somebody remembered.
    ///
    /// Added in S38, and it is also this enum's proptest strategy
    /// (`crate::arb::arbitrary_from_list`).
    pub const ALL: [Self; 8] = [
        Self::Empty,
        Self::GoForward,
        Self::GoBack,
        Self::LearnSpeed,
        Self::Off,
        Self::On,
        Self::Flash,
        Self::Toggle,
    ];
}

#[cfg(any(test, feature = "proptest"))]
crate::arb::arbitrary_from_list!(ExecutorButtonFunction);
