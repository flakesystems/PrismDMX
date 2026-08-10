//! The programmer — what the operator has touched but not yet stored.
//!
//! The programmer is **sparse**: an attribute that was never touched is absent,
//! not zero. That distinction is the whole point of the layer, because the merge
//! in `prism-engine` treats an absent attribute as "let the playbacks decide"
//! and a present one as an absolute override.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{AttributeType, FeatureGroup, FixtureId, PresetId};

/// Where a programmer value came from.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum ProgrammerValueSource {
    /// Set directly, by encoder or command line.
    #[default]
    Manual,
    /// Applied from a preset, and still linked to it.
    Preset,
    /// Pulled back out of a running playback.
    Recalled,
}

/// Stage of the three-stage Clear button (`CLAUDE.md`).
///
/// The wire form is the number both the console and the UI count with, so this
/// serialises as `0`, `1` or `2` rather than as a name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum ClearStage {
    /// Nothing cleared yet.
    #[default]
    Idle,
    /// First press: programmer values cleared, selection kept.
    ValuesCleared,
    /// Second press: selection cleared as well.
    SelectionCleared,
}

impl ClearStage {
    /// The wire number for this stage.
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::ValuesCleared => 1,
            Self::SelectionCleared => 2,
        }
    }
}

impl From<ClearStage> for u8 {
    fn from(stage: ClearStage) -> Self {
        stage.as_u8()
    }
}

impl TryFrom<u8> for ClearStage {
    type Error = InvalidClearStage;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Idle),
            1 => Ok(Self::ValuesCleared),
            2 => Ok(Self::SelectionCleared),
            other => Err(InvalidClearStage(other)),
        }
    }
}

/// Returned when a wire value outside `0..=2` is offered as a [`ClearStage`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidClearStage(pub u8);

impl core::fmt::Display for InvalidClearStage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "clear stage must be 0, 1 or 2, got {}", self.0)
    }
}

impl core::error::Error for InvalidClearStage {}

impl Serialize for ClearStage {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(self.as_u8())
    }
}

impl<'de> Deserialize<'de> for ClearStage {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = u8::deserialize(deserializer)?;
        Self::try_from(value).map_err(serde::de::Error::custom)
    }
}

/// One value the operator has set but not yet stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct ProgrammerValue {
    /// The value, `0..=65535`.
    pub value: u16,
    /// How it got here.
    pub source: ProgrammerValueSource,
    /// Link to the preset it came from, so a cue stored from the programmer
    /// keeps the preset reference.
    pub preset_ref: Option<PresetId>,
}

/// One touched value, as it travels on the wire.
///
/// The programmer is a nested map in Rust, because lookup by fixture and
/// attribute is what the merge does on every tick. It is a flat list on the
/// wire — see [`ProgrammerState::values`] for why.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct ProgrammerEntry {
    /// The fixture the value belongs to.
    pub fixture: FixtureId,
    /// The attribute that was touched.
    pub attribute: AttributeType,
    /// The value.
    pub value: ProgrammerValue,
}

/// Touched values, indexed by fixture and then attribute.
pub type ProgrammerValues = BTreeMap<FixtureId, BTreeMap<AttributeType, ProgrammerValue>>;

/// The programmer: selection plus the sparse set of touched values.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(rename_all = "camelCase")]
pub struct ProgrammerState {
    /// Selected fixtures, in selection order.
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(4)")
    )]
    pub selection: Vec<FixtureId>,
    /// The encoder bank the operator is working in.
    pub active_feature_group: FeatureGroup,
    /// Touched values. Absent means untouched — the programmer is sparse.
    ///
    /// Sent as a flat list of [`ProgrammerEntry`], not as a nested object.
    /// `Delta::ProgrammerChanged` is an internally tagged enum, and serde buffers
    /// the content of those before deserialising it; a map keyed by a number
    /// does not survive that buffering, because JSON object keys are strings and
    /// the buffered form no longer knows to read them back as numbers. A list of
    /// entries is also what a TypeScript `Map` is constructed from, so the shape
    /// in `ARCHITECTURE_SPEC.md` §6 is preserved on the client side.
    ///
    /// **Invariant:** no fixture maps to an empty attribute map. A fixture with
    /// nothing touched carries no information, has no representation in the flat
    /// list, and is therefore pruned rather than sent. [`Self::set_value`] and
    /// [`Self::clear_value`] maintain this; changing `values` directly must too.
    #[serde(with = "entry_list")]
    #[ts(as = "Vec<ProgrammerEntry>")]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "proptest::collection::btree_map(\
            proptest::prelude::any::<FixtureId>(), crate::arb::non_empty_map(3), 0..3)")
    )]
    pub values: ProgrammerValues,
    /// Stage of the Clear button.
    #[ts(type = "0 | 1 | 2")]
    pub clear_stage: ClearStage,
}

/// Serialises [`ProgrammerValues`] as a flat list of [`ProgrammerEntry`].
mod entry_list {
    use super::{ProgrammerEntry, ProgrammerValues};
    use serde::ser::SerializeSeq as _;
    use serde::{Deserialize as _, Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(
        values: &ProgrammerValues,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let count = values.values().map(|attributes| attributes.len()).sum();
        let mut seq = serializer.serialize_seq(Some(count))?;
        for (&fixture, attributes) in values {
            for (&attribute, &value) in attributes {
                seq.serialize_element(&ProgrammerEntry {
                    fixture,
                    attribute,
                    value,
                })?;
            }
        }
        seq.end()
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<ProgrammerValues, D::Error> {
        let entries = Vec::<ProgrammerEntry>::deserialize(deserializer)?;
        let mut values = ProgrammerValues::new();
        for entry in entries {
            values
                .entry(entry.fixture)
                .or_default()
                .insert(entry.attribute, entry.value);
        }
        Ok(values)
    }
}

impl ProgrammerState {
    /// Whether the operator has touched anything at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.selection.is_empty() && self.values.is_empty()
    }

    /// The value for one fixture and attribute, if it has been touched.
    #[must_use]
    pub fn value(&self, fixture: FixtureId, attribute: AttributeType) -> Option<&ProgrammerValue> {
        self.values.get(&fixture)?.get(&attribute)
    }

    /// Records a touched value, replacing any previous one.
    pub fn set_value(
        &mut self,
        fixture: FixtureId,
        attribute: AttributeType,
        value: ProgrammerValue,
    ) {
        self.values
            .entry(fixture)
            .or_default()
            .insert(attribute, value);
    }

    /// Forgets one touched value, returning it.
    ///
    /// Prunes the fixture entirely when that was its last touched attribute, so
    /// the sparse invariant on [`Self::values`] holds.
    pub fn clear_value(
        &mut self,
        fixture: FixtureId,
        attribute: AttributeType,
    ) -> Option<ProgrammerValue> {
        let attributes = self.values.get_mut(&fixture)?;
        let removed = attributes.remove(&attribute);
        if attributes.is_empty() {
            self.values.remove(&fixture);
        }
        removed
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        AttributeType, ClearStage, Delta, FeatureGroup, FixtureId, InvalidClearStage, PresetId,
        ProgrammerState, ProgrammerValue, ProgrammerValueSource,
    };
    use std::collections::BTreeMap;

    fn state() -> ProgrammerState {
        ProgrammerState {
            selection: vec![FixtureId::new(1), FixtureId::new(2)],
            active_feature_group: FeatureGroup::Color,
            values: BTreeMap::from([(
                FixtureId::new(1),
                BTreeMap::from([(
                    AttributeType::Blue,
                    ProgrammerValue {
                        value: 65535,
                        source: ProgrammerValueSource::Preset,
                        preset_ref: Some(PresetId::new(4)),
                    },
                )]),
            )]),
            clear_stage: ClearStage::Idle,
        }
    }

    #[test]
    fn programmer_state_matches_the_wire_shape() {
        assert_eq!(
            serde_json::to_string(&state()).unwrap(),
            r#"{"selection":[1,2],"activeFeatureGroup":"Color","values":[{"fixture":1,"attribute":"Blue","value":{"value":65535,"source":"Preset","presetRef":4}}],"clearStage":0}"#
        );
    }

    #[test]
    fn values_survive_a_round_trip_inside_an_internally_tagged_delta() {
        // The flat entry list exists for exactly this: a delta buffers its
        // content before deserialising, and a number-keyed map does not survive
        // that. Asserting it here keeps the reason attached to the shape.
        let delta = Delta::ProgrammerChanged { state: state() };
        let json = serde_json::to_string(&delta).unwrap();
        assert_eq!(serde_json::from_str::<Delta>(&json).unwrap(), delta);

        let packed = rmp_serde::to_vec_named(&delta).unwrap();
        assert_eq!(rmp_serde::from_slice::<Delta>(&packed).unwrap(), delta);
    }

    #[test]
    fn a_flat_entry_list_rebuilds_the_nested_index() {
        let json = serde_json::to_string(&state()).unwrap();
        let back: ProgrammerState = serde_json::from_str(&json).unwrap();
        assert_eq!(back, state());
        assert_eq!(
            back.value(FixtureId::new(1), AttributeType::Blue)
                .map(|value| value.value),
            Some(65535)
        );
    }

    #[test]
    fn an_empty_programmer_holds_no_values_at_all() {
        let empty = ProgrammerState::default();
        assert!(empty.is_empty());
        assert!(empty.selection.is_empty());
        assert_eq!(empty.clear_stage, ClearStage::Idle);
        assert!(!state().is_empty());
    }

    #[test]
    fn a_touched_attribute_is_present_and_an_untouched_one_is_absent() {
        let state = state();
        assert!(
            state
                .value(FixtureId::new(1), AttributeType::Blue)
                .is_some()
        );
        assert!(state.value(FixtureId::new(1), AttributeType::Red).is_none());
        assert!(
            state
                .value(FixtureId::new(2), AttributeType::Blue)
                .is_none()
        );
    }

    #[test]
    fn clearing_the_last_attribute_prunes_the_fixture() {
        // Otherwise the state would carry a fixture with nothing touched, which
        // has no representation on the wire and would come back pruned anyway.
        let mut state = state();
        assert_eq!(
            state
                .clear_value(FixtureId::new(1), AttributeType::Blue)
                .map(|value| value.value),
            Some(65535)
        );
        assert!(state.values.is_empty());
        assert!(
            state
                .clear_value(FixtureId::new(1), AttributeType::Blue)
                .is_none()
        );
    }

    #[test]
    fn setting_a_value_replaces_the_previous_one() {
        let mut state = ProgrammerState::default();
        let manual = ProgrammerValue {
            value: 100,
            source: ProgrammerValueSource::Manual,
            preset_ref: None,
        };
        state.set_value(FixtureId::new(1), AttributeType::Dimmer, manual);
        state.set_value(
            FixtureId::new(1),
            AttributeType::Dimmer,
            ProgrammerValue {
                value: 200,
                ..manual
            },
        );
        assert_eq!(
            state
                .value(FixtureId::new(1), AttributeType::Dimmer)
                .map(|value| value.value),
            Some(200)
        );
        assert_eq!(state.values.len(), 1);
    }

    #[test]
    fn clear_stage_is_zero_one_or_two_on_the_wire() {
        // CLAUDE.md: the Clear button is a three-stage machine, and the wire
        // form is the number the console and the UI both count with.
        for (stage, text) in [
            (ClearStage::Idle, "0"),
            (ClearStage::ValuesCleared, "1"),
            (ClearStage::SelectionCleared, "2"),
        ] {
            assert_eq!(serde_json::to_string(&stage).unwrap(), text);
            assert_eq!(serde_json::from_str::<ClearStage>(text).unwrap(), stage);
            assert_eq!(u8::from(stage).to_string(), text);
            assert_eq!(ClearStage::try_from(stage.as_u8()).unwrap(), stage);
        }
        assert!(serde_json::from_str::<ClearStage>("3").is_err());
        assert_eq!(ClearStage::try_from(3), Err(InvalidClearStage(3)));
        assert_eq!(
            InvalidClearStage(3).to_string(),
            "clear stage must be 0, 1 or 2, got 3"
        );
    }

    #[test]
    fn programmer_value_sources_are_named() {
        for (source, text) in [
            (ProgrammerValueSource::Manual, "\"Manual\""),
            (ProgrammerValueSource::Preset, "\"Preset\""),
            (ProgrammerValueSource::Recalled, "\"Recalled\""),
        ] {
            assert_eq!(serde_json::to_string(&source).unwrap(), text);
        }
    }
}
