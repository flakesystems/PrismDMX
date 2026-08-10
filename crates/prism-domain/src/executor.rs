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

/// What one of an executor's four buttons does.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
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
    /// Whether the sequence is currently running.
    pub is_active: bool,
    /// Index of the current cue within the sequence, if one is active.
    pub current_cue_index: Option<u32>,
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
    #[must_use]
    pub const fn from_page_and_slot(page: u32, slot: u32) -> Self {
        Self::new(page * EXECUTORS_PER_PAGE + slot)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Executor, ExecutorButtonFunction, ExecutorEncoderFunction, ExecutorFaderFunction,
        ExecutorId, SequenceId,
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
            is_active: true,
            current_cue_index: Some(0),
        }
    }

    #[test]
    fn executor_matches_the_wire_shape() {
        assert_eq!(
            serde_json::to_string(&executor()).unwrap(),
            r#"{"id":9,"sequenceId":1,"faderFunction":"Master","buttonFunctions":["Go+","Go-"],"encoderFunction":"Speed","masterLevel":65535,"isActive":true,"currentCueIndex":0}"#
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
