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

/// Which cue the programmer is editing, and whether it has moved since (S39).
///
/// **The update state.** `Command::EditCue` loads a cue into the programmer and
/// puts one of these in `Session::editing_cue`; `Command::Update` stores it back
/// and clears [`Self::modified`]. What it is *for* is the Update key: a desk
/// blinks it when there is an edit to put back, which is exactly
/// `editing_cue.is_some() && modified`.
///
/// It is session state rather than programmer state because it is **operating**
/// state in `ARCHITECTURE_SPEC.md` §4.1's sense: every attached client has to
/// blink the same key, and a second screen that worked it out from its own
/// mirror of the programmer would have to know which cue that programmer came
/// from — which is precisely the fact this carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct CueEdit {
    /// The sequence the cue is in.
    pub sequence_id: SequenceId,
    /// The cue, by its number.
    ///
    /// The number and not an index, for `Command::DeleteCue`'s reason: a number
    /// is what an operator wrote on a running order, and an index moves when a
    /// cue is inserted above it. A renumber of the cue being edited **carries
    /// this with it** — see `prism_core::ShowFile`.
    pub cue_number: String,
    /// Whether the programmer has changed since the cue was loaded.
    ///
    /// False the moment `EditCue` lands and false again after an `Update`. Any
    /// programmer edit in between sets it, which is what makes the key blink.
    pub modified: bool,
}

impl CueEdit {
    /// Whether this edit is of that cue of that sequence.
    ///
    /// The number is **trimmed on both sides**, because an operator typed one of
    /// them: `" 2 "` and `"2"` are the same cue everywhere else in the desk
    /// (`prism_core::Show::cue`), and an update state that did not agree would
    /// survive a `DeleteCue` of the very cue it names.
    #[must_use]
    pub fn is_of(&self, sequence: SequenceId, number: &str) -> bool {
        self.sequence_id == sequence && self.cue_number.trim() == number.trim()
    }
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
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::seconds()")
    )]
    pub fade_in: f64,
    /// Fade-out time in seconds.
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::seconds()")
    )]
    pub fade_out: f64,
    /// Delay before the fade starts, in seconds.
    #[serde(with = "crate::finite")]
    #[ts(as = "f64")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::seconds()")
    )]
    pub delay: f64,
    /// What starts this cue.
    pub trigger: CueTrigger,
    /// Trigger time in seconds, for [`CueTrigger::Time`].
    #[serde(with = "crate::finite::option")]
    #[ts(as = "Option<f64>")]
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

/// One field of a cue, changed on its own.
///
/// # Why one field rather than a whole cue
///
/// A cue sheet edits a cell: a name, a fade time, a trigger. The obvious
/// alternative — a command carrying every editable field at once — makes a
/// client read the cue, change one member and send the rest back, which is a
/// read-modify-write over state the daemon owns. Two operators editing two
/// different columns of one cue would then each undo the other's edit, and
/// neither would have done anything wrong. So the command names the field.
///
/// [`Self::Parts`] is deliberately **not** here. What a cue *does* comes from
/// the programmer through `Command::StoreCue`; a client that sent values would
/// be authoring show content, which is the same rule that keeps channels out of
/// `Command::PatchFixture` and profiles out of `Command::EmbedFixtureType`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(tag = "t", rename_all_fields = "camelCase")]
pub enum CueProperty {
    /// The number an operator types to reach the cue.
    ///
    /// The number is the key a cue is filed under inside its sequence, so this
    /// is a rename of the key — refused when the sequence already has a cue with
    /// that number, for `Command::RenumberFixture`'s reason: replacing the other
    /// one would delete a look nobody asked to delete.
    Number {
        /// The new number, as typed.
        number: String,
    },
    /// What the cue is called.
    Name {
        /// The new name. May be empty: a cue is reached by its number.
        name: String,
    },
    /// Fade-in time in seconds.
    FadeIn {
        /// Seconds, never negative.
        #[serde(with = "crate::finite")]
        #[ts(as = "f64")]
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::seconds()")
        )]
        seconds: f64,
    },
    /// Fade-out time in seconds.
    FadeOut {
        /// Seconds, never negative.
        #[serde(with = "crate::finite")]
        #[ts(as = "f64")]
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::seconds()")
        )]
        seconds: f64,
    },
    /// Delay before the fade starts, in seconds.
    Delay {
        /// Seconds, never negative.
        #[serde(with = "crate::finite")]
        #[ts(as = "f64")]
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::seconds()")
        )]
        seconds: f64,
    },
    /// What starts the cue, and after how long.
    ///
    /// The two travel together because [`CueTrigger::Time`] is the only trigger
    /// `trigger_time` means anything for: setting them separately leaves a cue
    /// that is triggered by a time nobody has given yet, which is a state an
    /// operator would meet halfway through an edit.
    Trigger {
        /// The trigger.
        trigger: CueTrigger,
        /// Seconds, for [`CueTrigger::Time`]. `None` for the others.
        #[serde(with = "crate::finite::option")]
        #[ts(as = "Option<f64>")]
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "proptest::option::of(crate::arb::seconds())")
        )]
        trigger_time: Option<f64>,
    },
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
    use crate::{
        AttributeType, Cue, CuePart, CueProperty, CueTrigger, FixtureId, Sequence, SequenceId,
    };

    /// **The number is trimmed on both sides**, because an operator typed one
    /// of them: `" 2 "` and `"2"` are the same cue everywhere else in the desk,
    /// and an update state that did not agree would survive a `DeleteCue` of
    /// the very cue it names.
    #[test]
    fn a_cue_edit_is_of_the_cue_whatever_spacing_the_operator_typed() {
        let edit = crate::CueEdit {
            sequence_id: SequenceId::new(1),
            cue_number: " 1.5 ".to_owned(),
            modified: false,
        };
        assert!(edit.is_of(SequenceId::new(1), "1.5"));
        assert!(edit.is_of(SequenceId::new(1), "  1.5"));
        assert!(!edit.is_of(SequenceId::new(1), "1.50"));
        assert!(!edit.is_of(SequenceId::new(2), "1.5"));
    }

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

    /// A cue property is one field, tagged like every other message.
    #[test]
    fn a_cue_property_names_the_field_it_changes() {
        assert_eq!(
            serde_json::to_string(&CueProperty::Name {
                name: "Blackout".to_owned()
            })
            .unwrap(),
            r#"{"t":"Name","name":"Blackout"}"#
        );
        assert_eq!(
            serde_json::to_string(&CueProperty::FadeIn { seconds: 2.5 }).unwrap(),
            r#"{"t":"FadeIn","seconds":2.5}"#
        );
        assert_eq!(
            serde_json::to_string(&CueProperty::Trigger {
                trigger: CueTrigger::Time,
                trigger_time: Some(4.0),
            })
            .unwrap(),
            r#"{"t":"Trigger","trigger":"Time","triggerTime":4.0}"#
        );
    }

    /// The trigger and its time travel together, and a trigger that needs no
    /// time says so with a `null` rather than by leaving the field out.
    #[test]
    fn a_trigger_carries_its_time_or_a_null() {
        let json = serde_json::to_value(CueProperty::Trigger {
            trigger: CueTrigger::Go,
            trigger_time: None,
        })
        .unwrap();
        assert!(json["triggerTime"].is_null(), "{json}");
    }

    /// A fade time that is not a number never reaches a cue: `crate::finite`
    /// guards this field on the way in as well as on the way out, because
    /// MessagePack can carry a NaN faithfully and a cue with one would never
    /// finish fading.
    #[test]
    fn a_fade_time_that_is_not_a_number_is_refused_in_both_directions() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(
                serde_json::to_string(&CueProperty::FadeIn { seconds: value }).is_err(),
                "{value} was written"
            );
        }
        let packed = rmp_serde::to_vec_named(&serde_json::json!({
            "t": "FadeOut",
            "seconds": f64::INFINITY,
        }))
        .unwrap();
        assert!(rmp_serde::from_slice::<CueProperty>(&packed).is_err());
    }
}
