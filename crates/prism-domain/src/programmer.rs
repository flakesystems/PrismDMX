//! The programmer — what the operator has touched but not yet stored.
//!
//! The programmer is **sparse**: an attribute that was never touched is absent,
//! not zero. That distinction is the whole point of the layer, because the merge
//! in `prism-engine` treats an absent attribute as "let the playbacks decide"
//! and a present one as an absolute override.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{AttributeType, FeatureGroup, FixtureId, GroupId, PresetId};

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

/// What the next press of Clear will do — `docs/DMX_MERGE.md` §3.1.
///
/// # It is derived, not counted, and that is S43's change
///
/// Until S43 this was a **counter on the button**: a press advanced it whatever
/// the programmer held, and the cycle ran on past the end. So a Clear that had
/// emptied everything left the button reading *stage 2*, and a value laid in
/// afterwards did not move it back — the next press cleared the *selection* of
/// something the operator had just set. The owner's punch list (B2) describes
/// exactly that, and asks for two things: the button should say what there is
/// to clear, and it should not move when there is nothing.
///
/// So the stage is now read off the contents. It cannot be stale, because there
/// is nothing to keep in step: [`Nothing`](Self::Nothing) is a programmer with
/// nothing in it, and every other value names the first thing a press would
/// take away. `Programmer::clear` matches on this rather than on a remembered
/// number, and a press at `Nothing` is a press that changes nothing at all.
///
/// The wire form is still the number both the console and the interface count
/// with — `0`, `1`, `2` or `3` rather than a name — and `0` is still *the
/// button is at rest*. What changed is what `1` and `2` mean: they used to say
/// what had **been** cleared and they now say what **would** be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum ClearStage {
    /// There is nothing to clear. A press does nothing and the key is dark.
    #[default]
    Nothing,
    /// The next press clears the values and keeps the selection.
    Values,
    /// The values are gone; the next press drops the selection.
    Selection,
    /// Values and selection are gone; the next press puts the rest back to
    /// where a fresh programmer starts — the feature group, and the page the
    /// session keeps beside it.
    All,
}

impl ClearStage {
    /// The wire number for this stage.
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Nothing => 0,
            Self::Values => 1,
            Self::Selection => 2,
            Self::All => 3,
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
            0 => Ok(Self::Nothing),
            1 => Ok(Self::Values),
            2 => Ok(Self::Selection),
            3 => Ok(Self::All),
            other => Err(InvalidClearStage(other)),
        }
    }
}

/// Returned when a wire value outside `0..=3` is offered as a [`ClearStage`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidClearStage(pub u8);

impl core::fmt::Display for InvalidClearStage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "clear stage must be 0, 1, 2 or 3, got {}", self.0)
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
    /// Which groups are **on**, in the order they were pressed — S43, B27.
    ///
    /// # Why a selection needs to remember where it came from
    ///
    /// Pressing a group used to toggle each of its fixtures, so a group whose
    /// lamps were already selected *deselected* them — the owner's entry, and
    /// the gesture nobody wants: selecting a second group after a first one
    /// punched a hole in the first. A group is now a switch of its own. Pressing
    /// it adds its fixtures; pressing it again takes back only the ones **no
    /// other selected group and no direct pick is still holding**.
    ///
    /// That question cannot be answered from [`Self::selection`] alone, because
    /// a fixture in it says nothing about who put it there. This field and
    /// [`Self::manual_selection`] are that provenance, and they are the
    /// programmer's — **D3**: the desk answers *what is selected*, and a client
    /// that worked it out from a list of groups would give a different answer
    /// the moment somebody edited one.
    ///
    /// It is not a second source of truth for the selection: the selection is
    /// still the list, still what a store reads, and a group whose membership
    /// changes under a standing selection does not silently re-select anything.
    /// What this records is only *which switches are down*.
    #[serde(default)]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(4)")
    )]
    pub selected_groups: Vec<GroupId>,
    /// The fixtures picked directly rather than through a group — S43, B27.
    ///
    /// The other half of the provenance above. A fixture picked by hand is held
    /// by that pick, so turning a group off leaves it selected — which is what
    /// an operator means by having added it. Kept as a set rather than derived
    /// from *selection minus the groups*, because a fixture can legitimately be
    /// both: picked by hand **and** inside a group that is on, and the two must
    /// not cancel.
    #[serde(default)]
    #[cfg_attr(
        any(test, feature = "proptest"),
        proptest(strategy = "crate::arb::small_vec(4)")
    )]
    pub manual_selection: Vec<FixtureId>,
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
    /// What the next press of Clear will do.
    ///
    /// **Derived, and written by [`Self::restage`] on every commit** — see
    /// [`ClearStage`]. It is carried on the wire so an interface can colour the
    /// key without knowing the rule, and it is not a second source of truth,
    /// because nothing ever sets it by hand.
    #[ts(type = "0 | 1 | 2 | 3")]
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

    /// Drops the selection and the provenance behind it.
    ///
    /// The three fields go together or not at all: a `selected_groups` left
    /// standing over an empty selection would mean the next press of that group
    /// *deselected* fixtures that were never selected. Clear and the `Set`
    /// forms of both selection commands all come through here.
    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.selected_groups.clear();
        self.manual_selection.clear();
    }

    /// What the next press of Clear would take away — S43, B2.
    ///
    /// The whole rule, in one place and read off the contents: the first thing
    /// there is to clear is the first thing a press clears. A programmer that
    /// holds nothing and sits on the default bank has nothing to clear, and the
    /// key says so rather than cycling.
    ///
    /// The order is `docs/DMX_MERGE.md` §3.1's and is not arbitrary — values
    /// before selection, because an operator who has set the wrong level far
    /// more often wants the level back than the selection gone.
    #[must_use]
    pub fn stage(&self) -> ClearStage {
        if !self.values.is_empty() {
            ClearStage::Values
        } else if !self.selection.is_empty() {
            ClearStage::Selection
        } else if self.active_feature_group == FeatureGroup::default() {
            ClearStage::Nothing
        } else {
            ClearStage::All
        }
    }

    /// Brings [`Self::clear_stage`] into line with the contents.
    ///
    /// Called wherever a programmer state is built or changed, so the field can
    /// never disagree with what is in it.
    pub fn restage(&mut self) {
        self.clear_stage = self.stage();
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
        AttributeType, ClearStage, Delta, FeatureGroup, FixtureId, GroupId, InvalidClearStage,
        PresetId, ProgrammerState, ProgrammerValue, ProgrammerValueSource,
    };
    use std::collections::BTreeMap;

    fn state() -> ProgrammerState {
        ProgrammerState {
            selection: vec![FixtureId::new(1), FixtureId::new(2)],
            selected_groups: vec![GroupId::new(3)],
            manual_selection: vec![FixtureId::new(2)],
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
            clear_stage: ClearStage::Nothing,
        }
    }

    #[test]
    fn programmer_state_matches_the_wire_shape() {
        assert_eq!(
            serde_json::to_string(&state()).unwrap(),
            r#"{"selection":[1,2],"selectedGroups":[3],"manualSelection":[2],"activeFeatureGroup":"Color","values":[{"fixture":1,"attribute":"Blue","value":{"value":65535,"source":"Preset","presetRef":4}}],"clearStage":0}"#
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
        assert_eq!(empty.clear_stage, ClearStage::Nothing);
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
    fn the_clear_stage_is_a_number_from_zero_to_three_on_the_wire() {
        // The wire form is the number the console and the interface both count
        // with. **Four since S43**: the stage says what the next press would
        // clear rather than what the last one did, so *there is nothing to
        // clear* needed a value of its own — and it is 0, which is still what a
        // key at rest reads as.
        for (stage, text) in [
            (ClearStage::Nothing, "0"),
            (ClearStage::Values, "1"),
            (ClearStage::Selection, "2"),
            (ClearStage::All, "3"),
        ] {
            assert_eq!(serde_json::to_string(&stage).unwrap(), text);
            assert_eq!(serde_json::from_str::<ClearStage>(text).unwrap(), stage);
            assert_eq!(u8::from(stage).to_string(), text);
            assert_eq!(ClearStage::try_from(stage.as_u8()).unwrap(), stage);
        }
        assert!(serde_json::from_str::<ClearStage>("4").is_err());
        assert_eq!(ClearStage::try_from(4), Err(InvalidClearStage(4)));
        assert_eq!(
            InvalidClearStage(4).to_string(),
            "clear stage must be 0, 1, 2 or 3, got 4"
        );
    }

    #[test]
    fn the_stage_is_the_first_thing_there_is_to_clear() {
        // The whole of B2's rule, read off the contents. `restage` is what every
        // commit in `prism_core::Programmer` calls, so the carried field can
        // never disagree with this.
        let mut state = ProgrammerState::default();
        assert_eq!(state.stage(), ClearStage::Nothing);

        state.active_feature_group = FeatureGroup::Color;
        assert_eq!(state.stage(), ClearStage::All);

        state.selection.push(FixtureId::new(1));
        assert_eq!(state.stage(), ClearStage::Selection);

        state.set_value(
            FixtureId::new(1),
            AttributeType::Dimmer,
            ProgrammerValue {
                value: 100,
                source: ProgrammerValueSource::Manual,
                preset_ref: None,
            },
        );
        assert_eq!(state.stage(), ClearStage::Values);

        state.restage();
        assert_eq!(state.clear_stage, ClearStage::Values);
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
