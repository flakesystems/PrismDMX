//! Cues and sequences.
//!
//! Cue numbers are strings on purpose (`ARCHITECTURE_SPEC.md` §6): an operator
//! inserting a cue between 1 and 2 types `1.5`, and `1.5` must sort between them
//! without a float ever entering the show file.

use core::cmp::Ordering;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{AttributeType, FixtureId, PresetId, SequenceId};

/// What starts a cue.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum CueTrigger {
    /// Waits for a Go from the operator.
    #[default]
    Go,
    /// Starts when the previous cue has finished fading.
    Follow,
    /// Starts after `triggerTime` seconds.
    Time,
    /// Starts on an audio trigger. Deferred past V1 — see `IMPLEMENTATION_PLAN.md` S5.
    Sound,
}

/// One attribute value inside a cue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct CuePart {
    /// The fixture this part applies to.
    pub fixture: FixtureId,
    /// The attribute being set.
    pub attribute: AttributeType,
    /// The value, `0..=65535`.
    pub value: u16,
    /// Link to the preset this value came from, which keeps the cue
    /// live-updatable when the preset is edited later.
    pub preset_ref: Option<PresetId>,
}

/// One step of a sequence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct Cue {
    /// Cue number as typed, e.g. `1`, `1.5`, `2`. A string, so decimal ordering
    /// survives the show file unchanged.
    pub number: String,
    /// Operator-facing name.
    pub name: String,
    /// Fade-in time in seconds.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::seconds()")
    )]
    pub fade_in: f64,
    /// Fade-out time in seconds.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::seconds()")
    )]
    pub fade_out: f64,
    /// Delay before the fade starts, in seconds.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::seconds()")
    )]
    pub delay: f64,
    /// What starts this cue.
    pub trigger: CueTrigger,
    /// Trigger time in seconds, for [`CueTrigger::Time`].
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "proptest::option::of(crate::arb::seconds())")
    )]
    pub trigger_time: Option<f64>,
    /// The values this cue sets.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(4)")
    )]
    pub parts: Vec<CuePart>,
}

impl Cue {
    /// Orders two cue numbers the way an operator reads them: `1`, `1.5`, `2`,
    /// `10` — not the lexical order `1`, `1.5`, `10`, `2`.
    ///
    /// A number that does not parse sorts after every number that does, and ties
    /// break on the original text, so the order is total and stable.
    #[must_use]
    pub fn compare_numbers(left: &str, right: &str) -> Ordering {
        match (left.trim().parse::<f64>(), right.trim().parse::<f64>()) {
            (Ok(left_value), Ok(right_value)) => left_value.total_cmp(&right_value),
            (Ok(_), Err(_)) => Ordering::Less,
            (Err(_), Ok(_)) => Ordering::Greater,
            (Err(_), Err(_)) => left.cmp(right),
        }
    }
}

/// A cue list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct Sequence {
    /// Sequence number.
    pub id: SequenceId,
    /// Operator-facing name.
    pub name: String,
    /// The cues, in playback order.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(3)")
    )]
    pub cues: Vec<Cue>,
    /// Whether the last cue wraps back to the first.
    ///
    /// Named `looping` in Rust because `loop` is a keyword; the wire name stays
    /// `loop`, as specified.
    #[serde(rename = "loop")]
    pub looping: bool,
}

#[cfg(test)]
mod tests {
    use crate::{AttributeType, Cue, CuePart, CueTrigger, FixtureId, Sequence, SequenceId};

    fn cue(number: &str) -> Cue {
        Cue {
            number: number.to_owned(),
            name: "Look".to_owned(),
            fade_in: 3.0,
            fade_out: 3.0,
            delay: 0.0,
            trigger: CueTrigger::Go,
            trigger_time: None,
            parts: vec![CuePart {
                fixture: FixtureId::new(1),
                attribute: AttributeType::Dimmer,
                value: 65535,
                preset_ref: None,
            }],
        }
    }

    #[test]
    fn cue_matches_the_wire_shape() {
        let json = serde_json::to_value(cue("1.5")).unwrap();
        let keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            [
                "delay",
                "fadeIn",
                "fadeOut",
                "name",
                "number",
                "parts",
                "trigger",
                "triggerTime",
            ]
        );
        assert_eq!(json["number"], "1.5");
        assert_eq!(json["trigger"], "Go");
    }

    #[test]
    fn all_four_trigger_types_exist() {
        for (trigger, text) in [
            (CueTrigger::Go, "\"Go\""),
            (CueTrigger::Follow, "\"Follow\""),
            (CueTrigger::Time, "\"Time\""),
            (CueTrigger::Sound, "\"Sound\""),
        ] {
            assert_eq!(serde_json::to_string(&trigger).unwrap(), text);
        }
    }

    #[test]
    fn cue_numbers_sort_by_decimal_value_not_lexically() {
        let mut numbers = ["10", "2", "1.5", "1"];
        numbers.sort_by(|a, b| Cue::compare_numbers(a, b));
        assert_eq!(numbers, ["1", "1.5", "2", "10"]);
    }

    #[test]
    fn an_unparsable_cue_number_sorts_last_instead_of_panicking() {
        let mut numbers = ["2", "oops", "1", "later"];
        numbers.sort_by(|a, b| Cue::compare_numbers(a, b));
        assert_eq!(numbers, ["1", "2", "later", "oops"]);
    }

    #[test]
    fn a_sequence_keeps_its_cues_in_order_and_may_loop() {
        let sequence = Sequence {
            id: SequenceId::new(1),
            name: "Main".to_owned(),
            cues: vec![cue("1"), cue("2")],
            looping: true,
        };
        let json = serde_json::to_value(&sequence).unwrap();
        assert_eq!(json["loop"], true);
        assert_eq!(json["cues"].as_array().unwrap().len(), 2);
    }
}
