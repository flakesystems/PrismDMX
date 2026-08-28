//! The home layer: what the merge is merging *into*.
//!
//! `docs/DMX_MERGE.md` §1: the bottom of the priority stack is never empty.
//! Every patched attribute has a home value, so an attribute with no active
//! source resolves to a defined state — a moving head with nothing running sits
//! at its home position rather than slammed to pan 0.
//!
//! A [`MergePlan`] is that layer, flattened once from the patch: one slot per
//! fixture and attribute, carrying the merge mode and the home value. It is
//! built before the tick starts and never changes while it runs, which is what
//! lets every buffer above it be sized up front (`ARCHITECTURE_SPEC.md` §3.1).

use core::fmt;
use std::collections::BTreeSet;

use prism_domain::{AttributeType, FeatureGroup, FixtureId, FixtureType, MergeMode};

/// Upper bound on attributes in one plan.
///
/// Not a product limit: 64 universes of 512 channels cannot hold more than
/// 32 768 sixteen-bit attributes, so this is comfortably above anything a real
/// patch reaches. It exists so that a corrupt patch becomes a rejected plan
/// rather than a multi-gigabyte allocation — the source set above it is sized
/// per source *and* per slot, so a wrong number here is expensive twice over.
pub const MAX_SLOTS: usize = 65_536;

/// Why a patch cannot be merged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeError {
    /// The same fixture number was patched twice.
    DuplicateFixture(FixtureId),
    /// A fixture type defines one attribute twice, which would give one
    /// physical parameter two slots and the encoder two answers.
    DuplicateAttribute {
        /// The fixture the type was patched as.
        fixture: FixtureId,
        /// The attribute defined twice.
        attribute: AttributeType,
    },
    /// More attributes than [`MAX_SLOTS`]. Carries the count the build had
    /// reached when it stopped, which is one past the limit — building the rest
    /// of an absurd patch just to report its exact size would be the allocation
    /// the limit exists to prevent.
    TooManySlots(usize),
    /// More playback sources than [`crate::MAX_SOURCES`].
    TooManySources(usize),
}

impl fmt::Display for MergeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateFixture(id) => write!(f, "fixture {id} is patched twice"),
            Self::DuplicateAttribute { fixture, attribute } => {
                write!(
                    f,
                    "fixture {fixture} defines the attribute {attribute:?} twice"
                )
            }
            Self::TooManySlots(count) => {
                write!(f, "{count} attributes exceeds the limit of {MAX_SLOTS}")
            }
            Self::TooManySources(count) => {
                let limit = crate::playback::MAX_SOURCES;
                write!(f, "{count} playback sources exceeds the limit of {limit}")
            }
        }
    }
}

impl std::error::Error for MergeError {}

/// One attribute of one fixture: the unit the merge resolves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttributeSlot {
    /// Which fixture.
    pub fixture: FixtureId,
    /// Which attribute of it.
    pub attribute: AttributeType,
    /// How playbacks combine for this attribute — from `AttributeDef.mergeMode`,
    /// which is the authority, not the attribute type's default.
    pub merge_mode: MergeMode,
    /// Which encoder bank the definition files this attribute under — from
    /// `AttributeDef.featureGroup`, again the authority rather than the
    /// attribute type's default. This is what decides whether the masters may
    /// touch the slot; see [`Self::is_intensity`].
    pub feature_group: FeatureGroup,
    /// The value this attribute takes when no source provides one.
    pub home: u16,
}

impl AttributeSlot {
    /// Whether the masters may scale this slot.
    ///
    /// `docs/DMX_MERGE.md` §4: group and grand masters scale **intensity only**,
    /// because a grand master that dimmed colour would desaturate the rig on the
    /// way down instead of dimming it. What counts as intensity is
    /// [`Self::feature_group`] — the attribute *definition*, which a profile sets
    /// per attribute — and deliberately not the attribute's name and not its
    /// merge mode. A profile is free to file an oddly-wired dimmer channel under
    /// [`FeatureGroup::Beam`], and then it is not intensity, whatever it is
    /// called.
    #[must_use]
    pub const fn is_intensity(&self) -> bool {
        matches!(self.feature_group, FeatureGroup::Dimmer)
    }
}

/// The flattened patch the merge resolves against.
///
/// Slots are ordered by fixture and then by attribute, so a plan is a function
/// of the patch rather than of the order the patch happened to be iterated in.
/// Two daemons given the same show agree on every slot index, which is what
/// lets S4's encoder and S6's programmer address slots by number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergePlan {
    slots: Box<[AttributeSlot]>,
}

impl MergePlan {
    /// Flattens a patch into slots.
    ///
    /// Each entry is a patched fixture number, the type it instantiates, and
    /// whether the desk supplies that fixture's intensity
    /// (`prism_domain::Fixture::has_software_dimmer`). The merge needs nothing
    /// else from the patch: addresses and inverts are S4's, geometry is the
    /// viewer's.
    ///
    /// # The slot with no channel — S43
    ///
    /// A fixture whose profile has no intensity gets a [`AttributeType::Dimmer`]
    /// slot anyway, resting at nought. It is an ordinary slot in every way the
    /// merge cares about — playbacks write it, the masters scale it because it
    /// is [`FeatureGroup::Dimmer`], the programmer takes it over — and the one
    /// thing it has not got is a DMX channel: `encode.rs` uses it to scale the
    /// fixture's colour on the way out instead. Which is what an intensity
    /// **is** for a fixture whose only output is its colour.
    ///
    /// The flag is the caller's answer and not this function's, because the
    /// operator can switch it off per fixture and the plan must follow the
    /// patch rather than second-guess it.
    ///
    /// # Errors
    ///
    /// [`MergeError`] if a fixture is patched twice, a fixture type names an
    /// attribute twice, or the patch is implausibly large.
    pub fn build<'a, I>(fixtures: I) -> Result<Self, MergeError>
    where
        I: IntoIterator<Item = (FixtureId, &'a FixtureType, bool)>,
    {
        let mut slots: Vec<AttributeSlot> = Vec::new();
        let mut patched: BTreeSet<FixtureId> = BTreeSet::new();
        for (fixture, fixture_type, software_dimmer) in fixtures {
            if !patched.insert(fixture) {
                return Err(MergeError::DuplicateFixture(fixture));
            }
            let mut defined: BTreeSet<AttributeType> = BTreeSet::new();
            if software_dimmer {
                if slots.len() == MAX_SLOTS {
                    return Err(MergeError::TooManySlots(MAX_SLOTS + 1));
                }
                defined.insert(AttributeType::Dimmer);
                slots.push(AttributeSlot {
                    fixture,
                    attribute: AttributeType::Dimmer,
                    merge_mode: AttributeType::Dimmer.default_merge_mode(),
                    feature_group: FeatureGroup::Dimmer,
                    // **Nought, and this is the whole point of the feature.** The
                    // colour underneath rests open (B1); what keeps the lamp off
                    // until somebody asks for it is this.
                    home: 0,
                });
            }
            for def in &fixture_type.attributes {
                if !defined.insert(def.attribute) {
                    return Err(MergeError::DuplicateAttribute {
                        fixture,
                        attribute: def.attribute,
                    });
                }
                if slots.len() == MAX_SLOTS {
                    return Err(MergeError::TooManySlots(MAX_SLOTS + 1));
                }
                slots.push(AttributeSlot {
                    fixture,
                    attribute: def.attribute,
                    merge_mode: def.merge_mode,
                    feature_group: def.feature_group,
                    home: def.default_value,
                });
            }
        }
        slots.sort_unstable_by_key(|slot| (slot.fixture, slot.attribute));
        Ok(Self {
            slots: slots.into_boxed_slice(),
        })
    }

    /// How many attributes this plan resolves.
    #[must_use]
    pub const fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// Whether nothing at all is patched. A legitimate state: a show that has
    /// just been created has no fixtures in it.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Every slot, in resolve order.
    #[must_use]
    pub const fn slots(&self) -> &[AttributeSlot] {
        &self.slots
    }

    /// One slot by index.
    #[must_use]
    pub fn slot(&self, index: usize) -> Option<&AttributeSlot> {
        self.slots.get(index)
    }

    /// The slot index of one attribute of one fixture, if it is patched.
    ///
    /// A binary search over the sorted slots, so a caller resolving a command
    /// does not walk the patch.
    #[must_use]
    pub fn index_of(&self, fixture: FixtureId, attribute: AttributeType) -> Option<usize> {
        self.slots
            .binary_search_by_key(&(fixture, attribute), |slot| (slot.fixture, slot.attribute))
            .ok()
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use crate::plan::{MAX_SLOTS, MergeError, MergePlan};
    use crate::testkit::{attribute_at, attribute_def, fixture_type, moving_head};
    use prism_domain::{AttributeDef, AttributeType, FeatureGroup, FixtureId, MergeMode};

    fn plan_of(entries: &[(u32, &prism_domain::FixtureType)]) -> MergePlan {
        MergePlan::build(
            entries
                .iter()
                .map(|(id, fixture_type)| (FixtureId::new(*id), *fixture_type, false)),
        )
        .unwrap()
    }

    fn slot_of(plan: &MergePlan, fixture: u32, attribute: AttributeType) -> usize {
        plan.index_of(FixtureId::new(fixture), attribute).unwrap()
    }

    #[test]
    fn a_plan_has_one_slot_per_fixture_and_attribute() {
        let head = moving_head();
        let plan = plan_of(&[(1, &head), (2, &head)]);
        assert_eq!(plan.slot_count(), 4);
        assert!(!plan.is_empty());
    }

    #[test]
    fn a_slot_carries_the_feature_group_its_definition_files_it_under() {
        // The masters scale intensity and nothing else (`docs/DMX_MERGE.md` §4),
        // so the plan has to answer "is this intensity?" without asking the
        // encoder or guessing from a name.
        let head = moving_head();
        let plan = plan_of(&[(1, &head)]);
        let dimmer = plan.slot(slot_of(&plan, 1, AttributeType::Dimmer)).unwrap();
        assert_eq!(dimmer.feature_group, FeatureGroup::Dimmer);
        assert!(dimmer.is_intensity());
        let pan = plan.slot(slot_of(&plan, 1, AttributeType::Pan)).unwrap();
        assert_eq!(pan.feature_group, FeatureGroup::Position);
        assert!(!pan.is_intensity());
    }

    #[test]
    fn intensity_is_what_the_definition_says_it_is_not_what_the_attribute_is_called() {
        // `docs/DMX_MERGE.md` §4 says masters scale intensity; which attributes
        // those are is a property of `AttributeDef`, which a profile may set per
        // attribute. A profile that files its dimmer under Beam - a haze machine
        // whose "dimmer" channel is a fan speed - must not be dimmed by the grand
        // master, and one that files a second intensity under Dimmer must be.
        let odd = fixture_type(
            "test.odd",
            vec![
                AttributeDef {
                    feature_group: FeatureGroup::Beam,
                    ..attribute_def(AttributeType::Dimmer, 0)
                },
                AttributeDef {
                    feature_group: FeatureGroup::Dimmer,
                    ..attribute_def(AttributeType::Shutter, 0)
                },
            ],
        );
        let plan = plan_of(&[(1, &odd)]);
        let named_dimmer = plan.slot(slot_of(&plan, 1, AttributeType::Dimmer)).unwrap();
        assert!(!named_dimmer.is_intensity());
        let named_shutter = plan
            .slot(slot_of(&plan, 1, AttributeType::Shutter))
            .unwrap();
        assert!(named_shutter.is_intensity());
    }

    #[test]
    fn a_slot_carries_the_home_value_and_merge_mode_from_the_fixture_type() {
        let head = moving_head();
        let plan = plan_of(&[(1, &head)]);
        let dimmer = plan
            .index_of(FixtureId::new(1), AttributeType::Dimmer)
            .unwrap();
        let pan = plan
            .index_of(FixtureId::new(1), AttributeType::Pan)
            .unwrap();

        let dimmer = plan.slot(dimmer).unwrap();
        assert_eq!(dimmer.home, 0);
        assert_eq!(dimmer.merge_mode, MergeMode::Htp);
        let pan = plan.slot(pan).unwrap();
        assert_eq!(pan.home, 32_768);
        assert_eq!(pan.merge_mode, MergeMode::Ltp);
    }

    #[test]
    fn slots_are_ordered_by_fixture_then_attribute_whatever_order_the_patch_arrives_in() {
        // A plan is a function of the patch, not of the order the patch was
        // iterated in: two clients that patch the same rig get the same slot
        // indices, and S4's encoder can rely on that.
        let head = moving_head();
        let forward = plan_of(&[(1, &head), (7, &head)]);
        let backward = plan_of(&[(7, &head), (1, &head)]);
        assert_eq!(forward.slots(), backward.slots());
        assert_eq!(forward.slot(0).unwrap().fixture, FixtureId::new(1));
        assert_eq!(forward.slot(0).unwrap().attribute, AttributeType::Dimmer);
        assert_eq!(forward.slot(3).unwrap().fixture, FixtureId::new(7));
        assert_eq!(forward.slot(3).unwrap().attribute, AttributeType::Pan);
    }

    #[test]
    fn an_unpatched_fixture_or_attribute_has_no_slot() {
        let head = moving_head();
        let plan = plan_of(&[(1, &head)]);
        assert!(
            plan.index_of(FixtureId::new(2), AttributeType::Pan)
                .is_none()
        );
        assert!(
            plan.index_of(FixtureId::new(1), AttributeType::Tilt)
                .is_none()
        );
        assert!(plan.slot(plan.slot_count()).is_none());
    }

    #[test]
    fn a_show_with_nothing_patched_is_a_valid_empty_plan() {
        // Unlike a frame layout, which needs at least one universe to output
        // into, an empty patch is an ordinary state: a show that has just been
        // created has no fixtures in it yet.
        let plan = MergePlan::build(std::iter::empty()).unwrap();
        assert_eq!(plan.slot_count(), 0);
        assert!(plan.is_empty());
        assert!(plan.slots().is_empty());
    }

    /// **A fixture with no intensity of its own gets a slot for one** — S43.
    ///
    /// Resting at nought, filed under the intensity bank so the masters scale
    /// it, and merged HTP like any dimmer. The colour beside it rests **open**
    /// (punch-list B1), so nought here is what keeps the lamp off.
    #[test]
    fn a_supplied_intensity_is_a_slot_that_rests_at_nought() {
        let par = fixture_type(
            "test.par",
            vec![
                attribute_at(AttributeType::Red, u16::MAX, 0, None),
                attribute_at(AttributeType::Green, u16::MAX, 1, None),
            ],
        );
        let plan = MergePlan::build([(FixtureId::new(1), &par, true)]).unwrap();
        assert_eq!(plan.slot_count(), 3, "two colours and the supplied dimmer");

        let index = plan
            .index_of(FixtureId::new(1), AttributeType::Dimmer)
            .expect("the desk supplied one");
        let slot = plan.slot(index).unwrap();
        assert_eq!(slot.home, 0, "dark at home");
        assert_eq!(slot.feature_group, FeatureGroup::Dimmer);
        assert_eq!(slot.merge_mode, MergeMode::Htp);
        assert!(slot.is_intensity(), "so the masters may scale it");

        // And the colour it sits over is untouched: open, as B1 left it.
        let red = plan
            .slot(
                plan.index_of(FixtureId::new(1), AttributeType::Red)
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(red.home, u16::MAX);
    }

    /// Switched off, the plan is what it was: no slot, and nothing renumbered
    /// around it.
    #[test]
    fn a_fixture_that_was_not_given_one_has_the_slots_it_always_had() {
        let par = fixture_type(
            "test.par",
            vec![attribute_at(AttributeType::Red, u16::MAX, 0, None)],
        );
        let plan = MergePlan::build([(FixtureId::new(1), &par, false)]).unwrap();
        assert_eq!(plan.slot_count(), 1);
        assert!(
            plan.index_of(FixtureId::new(1), AttributeType::Dimmer)
                .is_none()
        );
    }

    #[test]
    fn the_same_fixture_number_patched_twice_is_rejected() {
        let head = moving_head();
        let error = MergePlan::build([
            (FixtureId::new(4), &head, false),
            (FixtureId::new(4), &head, false),
        ])
        .unwrap_err();
        assert_eq!(error, MergeError::DuplicateFixture(FixtureId::new(4)));
    }

    #[test]
    fn a_fixture_type_naming_the_same_attribute_twice_is_rejected() {
        // Two definitions for one attribute would give the merge two slots for
        // one physical parameter and the encoder two answers for one channel.
        let broken = fixture_type(
            "test.broken",
            vec![
                attribute_def(AttributeType::Pan, 0),
                attribute_def(AttributeType::Pan, 100),
            ],
        );
        let error = MergePlan::build([(FixtureId::new(1), &broken, false)]).unwrap_err();
        assert_eq!(
            error,
            MergeError::DuplicateAttribute {
                fixture: FixtureId::new(1),
                attribute: AttributeType::Pan,
            }
        );
    }

    #[test]
    fn a_fixture_type_with_no_attributes_contributes_no_slots() {
        let empty = fixture_type("test.empty", Vec::new());
        let plan = plan_of(&[(1, &empty)]);
        assert_eq!(plan.slot_count(), 0);
    }

    #[test]
    fn an_absurd_patch_is_rejected_rather_than_allocated() {
        let wide = fixture_type(
            "test.wide",
            AttributeType::ALL
                .iter()
                .map(|attribute| attribute_def(*attribute, 0))
                .collect(),
        );
        let count = MAX_SLOTS / AttributeType::ALL.len() + 1;
        let fixtures: Vec<_> = (0..count as u32)
            .map(|id| (FixtureId::new(id), &wide, false))
            .collect();
        let error = MergePlan::build(fixtures).unwrap_err();
        assert!(matches!(error, MergeError::TooManySlots(_)), "{error:?}");
    }

    #[test]
    fn a_rejected_patch_says_why_in_words() {
        // These reach an operator through the patch dialogue.
        let errors = [
            (
                MergeError::DuplicateFixture(FixtureId::new(4)),
                "fixture 4 is patched twice",
            ),
            (
                MergeError::DuplicateAttribute {
                    fixture: FixtureId::new(4),
                    attribute: AttributeType::Pan,
                },
                "fixture 4 defines the attribute Pan twice",
            ),
            (
                MergeError::TooManySlots(99_999),
                "99999 attributes exceeds the limit of 65536",
            ),
            (
                MergeError::TooManySources(4_000),
                "4000 playback sources exceeds the limit of 1024",
            ),
        ];
        for (error, text) in errors {
            assert_eq!(error.to_string(), text);
            let as_error: &dyn std::error::Error = &error;
            assert_eq!(as_error.to_string(), text);
        }
    }
}
