//! The show: the patch, the profiles it uses, and everything stored on top.
//!
//! # Why the show embeds its fixture types
//!
//! [`Show::fixture_types`] is part of the show, not a reference into a profile
//! library sitting beside it. S1 recorded the reason on
//! `prism_domain::Command::PatchFixture`: if the library changes under a saved
//! show, every patched fixture silently changes meaning, and the operator finds
//! out when the rig does something else. A show that carries its own profiles
//! opens the same way on any machine and after any update.
//!
//! # Why the collections are maps rather than lists
//!
//! A `Delta::ShowPatch` carries JSON Pointers, and a pointer into an array
//! names a *position*: `/fixtures/3` is the fourth element, so removing the
//! first fixture silently renames every fixture after it. Keyed by
//! identifier, `/fixtures/3` is fixture 3 whatever else happens to the show.
//! The wire shape is therefore an object per collection, keyed by the
//! identifier the operator uses.
//!
//! # What is not show content
//!
//! [`Show::patch_revision`] and [`Show::is_dirty`] describe *this run of the
//! daemon*, not the show, and are excluded from the serialised form. A reloaded
//! show starts at revision 0 and clean.

use std::collections::BTreeMap;
use std::fmt;

use prism_domain::{
    Cue, Executor, ExecutorId, Fixture, FixtureType, Group, GroupId, JsonPatchOp, JsonValue,
    Preset, PresetId, Sequence, SequenceId, UniverseId,
};
use serde::{Deserialize, Serialize};

use crate::conflict::{PatchConflict, ShowIssue};

/// Wire name of the embedded profile library.
pub(crate) const FIXTURE_TYPES: &str = "fixtureTypes";
/// Wire name of the patch.
pub(crate) const FIXTURES: &str = "fixtures";
/// Wire name of the group pool.
pub(crate) const GROUPS: &str = "groups";
/// Wire name of the preset pools.
pub(crate) const PRESETS: &str = "presets";
/// Wire name of the sequence pool.
pub(crate) const SEQUENCES: &str = "sequences";
/// Wire name of the executor grid.
pub(crate) const EXECUTORS: &str = "executors";

/// Why a show edit was refused.
///
/// Every variant leaves the show exactly as it was: validation happens before
/// anything is written, which is what makes "a rejection changes nothing" a
/// property of the code rather than a promise in a comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShowError {
    /// A fixture type key that is not embedded in this show.
    UnknownFixtureType(String),
    /// A fixture number that is not patched.
    UnknownFixture(prism_domain::FixtureId),
    /// A group number that does not exist.
    UnknownGroup(GroupId),
    /// A preset number that does not exist.
    UnknownPreset(PresetId),
    /// A sequence number that does not exist.
    UnknownSequence(SequenceId),
    /// An executor number that does not exist.
    UnknownExecutor(ExecutorId),
    /// The executor exists but has no sequence to play.
    ExecutorHasNoSequence(ExecutorId),
    /// A fixture type must have a key: it is what a fixture references.
    EmptyFixtureTypeId,
    /// A footprint of zero channels controls nothing and cannot be addressed.
    EmptyFootprint(String),
    /// An attribute's coarse or fine channel lies outside the footprint it is
    /// supposed to fit inside, which would put it in the next fixture's
    /// channels.
    AttributeOutsideFootprint {
        /// The type that defines it.
        type_id: String,
        /// The attribute.
        attribute: prism_domain::AttributeType,
        /// The offending offset.
        offset: u16,
        /// The footprint it has to fit inside.
        footprint: u16,
    },
    /// A fixture type defines one attribute twice, which would give one
    /// physical parameter two values.
    DuplicateAttribute {
        /// The type that defines it.
        type_id: String,
        /// The attribute defined twice.
        attribute: prism_domain::AttributeType,
    },
    /// The fixture's footprint does not fit at its address: address 0, or a
    /// range running past channel 512.
    AddressOutOfRange {
        /// The fixture.
        fixture: prism_domain::FixtureId,
        /// Its start address.
        address: u16,
        /// The footprint of the type it instantiates.
        footprint: u16,
    },
    /// `ARCHITECTURE_SPEC.md` §6 gives universes the range `1..=64`.
    UniverseOutOfRange {
        /// The fixture.
        fixture: prism_domain::FixtureId,
        /// The universe it asks for.
        universe: UniverseId,
    },
    /// Replacing an embedded type would leave a fixture that uses it no longer
    /// fitting in its universe.
    TypeChangeBreaksPatch {
        /// The type being replaced.
        type_id: String,
        /// The first fixture that would stop fitting.
        fixture: prism_domain::FixtureId,
    },
    /// A fixture type cannot be removed while a fixture instantiates it.
    FixtureTypeInUse {
        /// The type.
        type_id: String,
        /// The first fixture using it.
        fixture: prism_domain::FixtureId,
    },
    /// A cue number is what an operator types to reach the cue, so it may not
    /// be blank.
    EmptyCueNumber,
    /// Two cues in one sequence carry the same number, so a Goto could not say
    /// which one it means.
    DuplicateCueNumber {
        /// The sequence.
        sequence: SequenceId,
        /// The number used twice.
        number: String,
    },
    /// An absolute attribute value outside `0..=65535`.
    ValueOutOfRange(i32),
    /// A session command reached the show applier. `ARCHITECTURE_SPEC.md` §4.4
    /// commands act on session state, which is S12.
    NotAShowCommand,
    /// The show could not be projected into JSON. `prism-domain` refuses
    /// non-finite floats in both directions, so this is what a NaN that reached
    /// the model looks like on the way out.
    NotRepresentable(String),
}

impl fmt::Display for ShowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFixtureType(id) => write!(f, "no fixture type {id:?} in this show"),
            Self::UnknownFixture(id) => write!(f, "fixture {id} is not patched"),
            Self::UnknownGroup(id) => write!(f, "no group {id}"),
            Self::UnknownPreset(id) => write!(f, "no preset {id}"),
            Self::UnknownSequence(id) => write!(f, "no sequence {id}"),
            Self::UnknownExecutor(id) => write!(f, "no executor {id}"),
            Self::ExecutorHasNoSequence(id) => write!(f, "executor {id} has no sequence"),
            Self::EmptyFixtureTypeId => write!(f, "a fixture type needs a key"),
            Self::EmptyFootprint(id) => write!(f, "fixture type {id:?} has no channels"),
            Self::AttributeOutsideFootprint {
                type_id,
                attribute,
                offset,
                footprint,
            } => write!(
                f,
                "fixture type {type_id:?} puts {attribute:?} at offset {offset}, \
                 outside its footprint of {footprint}"
            ),
            Self::DuplicateAttribute { type_id, attribute } => {
                write!(f, "fixture type {type_id:?} defines {attribute:?} twice")
            }
            Self::AddressOutOfRange {
                fixture,
                address,
                footprint,
            } => write!(
                f,
                "fixture {fixture} does not fit: {footprint} channels from address {address}"
            ),
            Self::UniverseOutOfRange { fixture, universe } => write!(
                f,
                "fixture {fixture} asks for universe {universe}, outside {}..={}",
                UniverseId::MIN,
                UniverseId::MAX
            ),
            Self::TypeChangeBreaksPatch { type_id, fixture } => write!(
                f,
                "replacing fixture type {type_id:?} would leave fixture {fixture} out of range"
            ),
            Self::FixtureTypeInUse { type_id, fixture } => {
                write!(f, "fixture type {type_id:?} is used by fixture {fixture}")
            }
            Self::EmptyCueNumber => write!(f, "a cue needs a number"),
            Self::DuplicateCueNumber { sequence, number } => {
                write!(f, "sequence {sequence} already has a cue {number:?}")
            }
            Self::ValueOutOfRange(value) => {
                write!(f, "attribute value {value} is outside 0..=65535")
            }
            Self::NotAShowCommand => write!(f, "this is a session command, not a show command"),
            Self::NotRepresentable(reason) => write!(f, "the show cannot be encoded: {reason}"),
        }
    }
}

impl core::error::Error for ShowError {}

/// Everything the daemon is authoritative about, apart from the session, the
/// programmer and the journal.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Show {
    /// The profiles this show uses, keyed by [`FixtureType::id`].
    fixture_types: BTreeMap<String, FixtureType>,
    /// The patch, keyed by fixture number.
    fixtures: BTreeMap<prism_domain::FixtureId, Fixture>,
    /// The group pool.
    groups: BTreeMap<GroupId, Group>,
    /// The preset pools. Keyed by number alone - see [`Show::store_preset`].
    presets: BTreeMap<PresetId, Preset>,
    /// The sequence pool.
    sequences: BTreeMap<SequenceId, Sequence>,
    /// The executor grid.
    executors: BTreeMap<ExecutorId, Executor>,
    /// Counts patch changes. Not show content - see the module documentation.
    #[serde(skip)]
    patch_revision: u64,
    /// Whether there are unsaved changes. Not show content.
    #[serde(skip)]
    dirty: bool,
}

impl Show {
    /// An empty show: no profiles, no patch, nothing stored.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    // -- queries ----------------------------------------------------------

    /// The profiles this show carries.
    pub fn fixture_types(&self) -> impl Iterator<Item = &FixtureType> {
        self.fixture_types.values()
    }

    /// One profile by key.
    #[must_use]
    pub fn fixture_type(&self, type_id: &str) -> Option<&FixtureType> {
        self.fixture_types.get(type_id)
    }

    /// The patch, in fixture-number order.
    pub fn fixtures(&self) -> impl Iterator<Item = &Fixture> {
        self.fixtures.values()
    }

    /// One patched fixture.
    #[must_use]
    pub fn fixture(&self, id: prism_domain::FixtureId) -> Option<&Fixture> {
        self.fixtures.get(&id)
    }

    /// The groups, in number order.
    pub fn groups(&self) -> impl Iterator<Item = &Group> {
        self.groups.values()
    }

    /// One group.
    #[must_use]
    pub fn group(&self, id: GroupId) -> Option<&Group> {
        self.groups.get(&id)
    }

    /// The presets, in number order.
    pub fn presets(&self) -> impl Iterator<Item = &Preset> {
        self.presets.values()
    }

    /// One preset.
    #[must_use]
    pub fn preset(&self, id: PresetId) -> Option<&Preset> {
        self.presets.get(&id)
    }

    /// The sequences, in number order.
    pub fn sequences(&self) -> impl Iterator<Item = &Sequence> {
        self.sequences.values()
    }

    /// One sequence.
    #[must_use]
    pub fn sequence(&self, id: SequenceId) -> Option<&Sequence> {
        self.sequences.get(&id)
    }

    /// The executors, in number order.
    pub fn executors(&self) -> impl Iterator<Item = &Executor> {
        self.executors.values()
    }

    /// One executor.
    #[must_use]
    pub fn executor(&self, id: ExecutorId) -> Option<&Executor> {
        self.executors.get(&id)
    }

    /// The patched fixtures paired with the profiles they instantiate.
    ///
    /// Exactly what `prism_engine::MergeBody::for_patch` consumes. A fixture
    /// whose type is missing is skipped rather than guessed at: it can only
    /// arise from a hand-edited file, and [`Show::issues`] reports it.
    pub fn patched(&self) -> impl Iterator<Item = (&Fixture, &FixtureType)> {
        self.fixtures.values().filter_map(|fixture| {
            self.fixture_types
                .get(&fixture.type_id)
                .map(|fixture_type| (fixture, fixture_type))
        })
    }

    /// The universes the patch occupies, ascending and without repeats.
    ///
    /// What a `prism_engine::FrameLayout` is built from: a universe nothing is
    /// patched into is a frame nobody writes.
    #[must_use]
    pub fn universes(&self) -> Vec<UniverseId> {
        let mut universes: Vec<UniverseId> = self
            .fixtures
            .values()
            .map(|fixture| fixture.universe)
            .collect();
        universes.sort_unstable();
        universes.dedup();
        universes
    }

    /// Counts changes to the patch or to the embedded profiles.
    ///
    /// The number a repatch moves. `prism-engine` addresses the programmer by
    /// merge-plan slot (S6), and that number is only meaningful against the
    /// patch it was derived from - so a daemon that has queued programmer
    /// commands must notice that this number changed, drop them, and rebuild
    /// the `MergeBody`. Rebuilding also means blanking the publisher's frame
    /// buffers, because the encoder leaves unpatched channels alone (S4).
    #[must_use]
    pub const fn patch_revision(&self) -> u64 {
        self.patch_revision
    }

    /// Whether the show has changes that have not been written to disk.
    ///
    /// Drives `Delta::DirtyFlag`, which is the console's Save LED.
    #[must_use]
    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Records that the show has been written to disk.
    ///
    /// Returns whether the flag actually changed, so the daemon only sends a
    /// `Delta::DirtyFlag` when there is news.
    pub const fn mark_saved(&mut self) -> bool {
        let changed = self.dirty;
        self.dirty = false;
        changed
    }

    /// Overlapping address ranges, in a stable order.
    ///
    /// Not an error: patching a second fixture onto the first is how an
    /// operator clones one, and S4 made the outcome deterministic rather than
    /// merely tolerated. See [`PatchConflict`].
    #[must_use]
    pub fn conflicts(&self) -> Vec<PatchConflict> {
        crate::conflict::conflicts(self)
    }

    /// Everything about this show that a patch sheet should show in red:
    /// overlaps, missing profiles and references to things that are not there.
    #[must_use]
    pub fn issues(&self) -> Vec<ShowIssue> {
        crate::conflict::issues(self)
    }

    // -- edits ------------------------------------------------------------

    /// Embeds a profile, or replaces one already embedded.
    ///
    /// # Errors
    ///
    /// [`ShowError::EmptyFixtureTypeId`], [`ShowError::EmptyFootprint`],
    /// [`ShowError::DuplicateAttribute`] or
    /// [`ShowError::AttributeOutsideFootprint`] if the profile does not
    /// describe an addressable fixture; [`ShowError::TypeChangeBreaksPatch`] if
    /// replacing it would leave a fixture that uses it running past the end of
    /// its universe.
    pub fn embed_fixture_type(
        &mut self,
        fixture_type: FixtureType,
    ) -> Result<Vec<JsonPatchOp>, ShowError> {
        check_fixture_type(&fixture_type)?;
        // A replacement may not break the patch that is already standing on it.
        for fixture in self.fixtures.values() {
            if fixture.type_id == fixture_type.id
                && fixture.last_address(fixture_type.footprint).is_none()
            {
                return Err(ShowError::TypeChangeBreaksPatch {
                    type_id: fixture_type.id.clone(),
                    fixture: fixture.id,
                });
            }
        }
        let path = pointer(FIXTURE_TYPES, &fixture_type.id);
        let op = put(
            path,
            &fixture_type,
            self.fixture_types.contains_key(&fixture_type.id),
        )?;
        self.fixture_types
            .insert(fixture_type.id.clone(), fixture_type);
        self.touch_patch();
        Ok(vec![op])
    }

    /// Removes an embedded profile.
    ///
    /// # Errors
    ///
    /// [`ShowError::UnknownFixtureType`] if it is not there, or
    /// [`ShowError::FixtureTypeInUse`] if a fixture instantiates it - a patch
    /// whose profile has gone is a fixture nobody can address.
    pub fn remove_fixture_type(&mut self, type_id: &str) -> Result<Vec<JsonPatchOp>, ShowError> {
        if !self.fixture_types.contains_key(type_id) {
            return Err(ShowError::UnknownFixtureType(type_id.to_owned()));
        }
        if let Some(fixture) = self
            .fixtures
            .values()
            .find(|fixture| fixture.type_id == type_id)
        {
            return Err(ShowError::FixtureTypeInUse {
                type_id: type_id.to_owned(),
                fixture: fixture.id,
            });
        }
        self.fixture_types.remove(type_id);
        self.touch_patch();
        Ok(vec![JsonPatchOp::Remove {
            path: pointer(FIXTURE_TYPES, type_id),
        }])
    }

    /// Patches a fixture, or repatches one already there.
    ///
    /// # Errors
    ///
    /// [`ShowError::UnknownFixtureType`] if the show does not carry the
    /// profile: a show is self-contained, so the profile is embedded first.
    /// [`ShowError::UniverseOutOfRange`] or [`ShowError::AddressOutOfRange`] if
    /// the fixture does not fit where it is asked to go.
    ///
    /// An address that overlaps another fixture is **not** an error. See
    /// [`Show::conflicts`].
    pub fn patch_fixture(&mut self, fixture: Fixture) -> Result<Vec<JsonPatchOp>, ShowError> {
        self.check_fixture(&fixture)?;
        let path = pointer(FIXTURES, &fixture.id.to_string());
        let op = put(path, &fixture, self.fixtures.contains_key(&fixture.id))?;
        self.fixtures.insert(fixture.id, fixture);
        self.touch_patch();
        Ok(vec![op])
    }

    /// Removes a fixture from the patch.
    ///
    /// Deliberately does **not** cascade into groups, presets or cues. A show
    /// outlives the rig it was written on: S5 already decided that a cue naming
    /// a fixture that is no longer patched is dropped when the sequence is
    /// compiled, not refused, and deleting an operator's stored looks because
    /// a light came out of the rig for one show would be worse than leaving
    /// them. [`Show::issues`] reports what is now dangling.
    ///
    /// # Errors
    ///
    /// [`ShowError::UnknownFixture`] if it is not patched.
    pub fn unpatch_fixture(
        &mut self,
        id: prism_domain::FixtureId,
    ) -> Result<Vec<JsonPatchOp>, ShowError> {
        if self.fixtures.remove(&id).is_none() {
            return Err(ShowError::UnknownFixture(id));
        }
        self.touch_patch();
        Ok(vec![JsonPatchOp::Remove {
            path: pointer(FIXTURES, &id.to_string()),
        }])
    }

    /// Stores a group, replacing one with the same number.
    ///
    /// # Errors
    ///
    /// [`ShowError::UnknownFixture`] if a member is not patched.
    pub fn store_group(&mut self, group: Group) -> Result<Vec<JsonPatchOp>, ShowError> {
        for fixture in &group.fixtures {
            self.require_fixture(*fixture)?;
        }
        let path = pointer(GROUPS, &group.id.to_string());
        let op = put(path, &group, self.groups.contains_key(&group.id))?;
        self.groups.insert(group.id, group);
        self.touch();
        Ok(vec![op])
    }

    /// Removes a group.
    ///
    /// # Errors
    ///
    /// [`ShowError::UnknownGroup`] if there is no such group.
    pub fn remove_group(&mut self, id: GroupId) -> Result<Vec<JsonPatchOp>, ShowError> {
        if self.groups.remove(&id).is_none() {
            return Err(ShowError::UnknownGroup(id));
        }
        self.touch();
        Ok(vec![JsonPatchOp::Remove {
            path: pointer(GROUPS, &id.to_string()),
        }])
    }

    /// Stores a preset, replacing one with the same number.
    ///
    /// Preset numbers are unique across pools, not per pool.
    /// `Command::ApplyPreset` carries a number and no pool, so a number that
    /// meant one thing in the colour pool and another in the position pool
    /// would make that command ambiguous. The pool is how a preset is filed and
    /// which encoder bank shows it; it is not part of its identity.
    ///
    /// # Errors
    ///
    /// [`ShowError::UnknownFixture`] if a stored value names a fixture that is
    /// not patched.
    pub fn store_preset(&mut self, preset: Preset) -> Result<Vec<JsonPatchOp>, ShowError> {
        for value in &preset.values {
            self.require_fixture(value.fixture)?;
        }
        let path = pointer(PRESETS, &preset.id.to_string());
        let op = put(path, &preset, self.presets.contains_key(&preset.id))?;
        self.presets.insert(preset.id, preset);
        self.touch();
        Ok(vec![op])
    }

    /// Removes a preset.
    ///
    /// A cue part that referenced it keeps its stored value and loses the link,
    /// which [`Show::issues`] reports. The alternative - editing every cue that
    /// referenced it - would change light the operator did not ask to change.
    ///
    /// # Errors
    ///
    /// [`ShowError::UnknownPreset`] if there is no such preset.
    pub fn remove_preset(&mut self, id: PresetId) -> Result<Vec<JsonPatchOp>, ShowError> {
        if self.presets.remove(&id).is_none() {
            return Err(ShowError::UnknownPreset(id));
        }
        self.touch();
        Ok(vec![JsonPatchOp::Remove {
            path: pointer(PRESETS, &id.to_string()),
        }])
    }

    /// Stores a sequence, replacing one with the same number.
    ///
    /// The cues are sorted into playback order by [`Cue::compare_numbers`], so
    /// `1`, `1.5`, `2`, `10` is the order the show holds them in whatever order
    /// they arrived. The delta carries the sorted list, so a client's mirror
    /// agrees.
    ///
    /// # Errors
    ///
    /// [`ShowError::EmptyCueNumber`], [`ShowError::DuplicateCueNumber`],
    /// [`ShowError::UnknownFixture`] for a part naming an unpatched fixture, or
    /// [`ShowError::UnknownPreset`] for a part linked to a preset that does not
    /// exist.
    pub fn store_sequence(&mut self, sequence: Sequence) -> Result<Vec<JsonPatchOp>, ShowError> {
        let mut sequence = sequence;
        self.check_cues(sequence.id, &sequence.cues)?;
        sequence
            .cues
            .sort_by(|left, right| Cue::compare_numbers(&left.number, &right.number));
        let path = pointer(SEQUENCES, &sequence.id.to_string());
        let op = put(path, &sequence, self.sequences.contains_key(&sequence.id))?;
        self.sequences.insert(sequence.id, sequence);
        self.touch();
        Ok(vec![op])
    }

    /// Removes a sequence.
    ///
    /// An executor that played it keeps pointing at a sequence that is gone,
    /// which [`Show::issues`] reports; clearing the executor is a separate
    /// operator decision.
    ///
    /// # Errors
    ///
    /// [`ShowError::UnknownSequence`] if there is no such sequence.
    pub fn remove_sequence(&mut self, id: SequenceId) -> Result<Vec<JsonPatchOp>, ShowError> {
        if self.sequences.remove(&id).is_none() {
            return Err(ShowError::UnknownSequence(id));
        }
        self.touch();
        Ok(vec![JsonPatchOp::Remove {
            path: pointer(SEQUENCES, &id.to_string()),
        }])
    }

    /// Stores one cue into a sequence, replacing the cue with that number.
    ///
    /// The delta replaces the whole cue list rather than describing an insert.
    /// A cue list is small, and inserting `1.5` between `1` and `2` shifts
    /// every index after it - so the precise operations would be a move of
    /// everything below, which is both longer and easier to get wrong than
    /// sending the list.
    ///
    /// # Errors
    ///
    /// [`ShowError::UnknownSequence`], [`ShowError::EmptyCueNumber`],
    /// [`ShowError::UnknownFixture`] or [`ShowError::UnknownPreset`].
    pub fn store_cue(
        &mut self,
        sequence_id: SequenceId,
        cue: Cue,
    ) -> Result<Vec<JsonPatchOp>, ShowError> {
        let Some(sequence) = self.sequences.get(&sequence_id) else {
            return Err(ShowError::UnknownSequence(sequence_id));
        };
        self.check_cues(sequence_id, core::slice::from_ref(&cue))?;

        // The new list is built beside the old one and only swapped in once the
        // operation that describes it exists. Editing in place and encoding
        // afterwards would leave the cue stored behind a returned error, which
        // is precisely what "a rejection changes nothing" forbids.
        let mut updated = sequence.clone();
        match updated
            .cues
            .iter_mut()
            .find(|existing| existing.number == cue.number)
        {
            Some(existing) => *existing = cue,
            None => updated.cues.push(cue),
        }
        updated
            .cues
            .sort_by(|left, right| Cue::compare_numbers(&left.number, &right.number));
        let op = put(pointer(SEQUENCES, &sequence_id.to_string()), &updated, true)?;
        self.sequences.insert(sequence_id, updated);
        self.touch();
        Ok(vec![op])
    }

    /// Stores an executor, replacing the one in that slot.
    ///
    /// # Errors
    ///
    /// [`ShowError::UnknownSequence`] if it is assigned a sequence that does
    /// not exist.
    pub fn store_executor(&mut self, executor: Executor) -> Result<Vec<JsonPatchOp>, ShowError> {
        if let Some(sequence) = executor.sequence_id
            && !self.sequences.contains_key(&sequence)
        {
            return Err(ShowError::UnknownSequence(sequence));
        }
        let path = pointer(EXECUTORS, &executor.id.to_string());
        let op = put(path, &executor, self.executors.contains_key(&executor.id))?;
        self.executors.insert(executor.id, executor);
        self.touch();
        Ok(vec![op])
    }

    /// Moves an executor's master.
    ///
    /// # Errors
    ///
    /// [`ShowError::UnknownExecutor`] if there is no such executor.
    pub fn set_executor_master(
        &mut self,
        id: ExecutorId,
        level: u16,
    ) -> Result<Vec<JsonPatchOp>, ShowError> {
        let Some(executor) = self.executors.get_mut(&id) else {
            return Err(ShowError::UnknownExecutor(id));
        };
        executor.master_level = level;
        self.touch();
        Ok(vec![JsonPatchOp::Replace {
            path: format!("{}/masterLevel", pointer(EXECUTORS, &id.to_string())),
            value: JsonValue::Int(i64::from(level)),
        }])
    }

    /// Records what the engine reports about a running executor.
    ///
    /// This travels as `Delta::ExecutorState` rather than as a show patch: it
    /// is playback state the engine owns, and the protocol gives it its own
    /// delta so a client does not have to diff the show to draw a moving
    /// executor bar.
    ///
    /// Returns whether anything actually changed.
    ///
    /// # Errors
    ///
    /// [`ShowError::UnknownExecutor`] if there is no such executor.
    pub fn record_executor_state(
        &mut self,
        id: ExecutorId,
        is_active: bool,
        cue_index: Option<u32>,
    ) -> Result<bool, ShowError> {
        let Some(executor) = self.executors.get_mut(&id) else {
            return Err(ShowError::UnknownExecutor(id));
        };
        let changed = executor.is_active != is_active || executor.current_cue_index != cue_index;
        executor.is_active = is_active;
        executor.current_cue_index = cue_index;
        // Deliberately not marked dirty: an executor running is not an unsaved
        // edit, and a Save LED that lit up because a cue advanced would tell
        // the operator nothing.
        Ok(changed)
    }

    /// The show as JSON, which is the shape a client mirrors and the shape the
    /// patch operations point into.
    ///
    /// # Errors
    ///
    /// [`ShowError::NotRepresentable`] if a value cannot be encoded - a
    /// non-finite float that reached the model, which `prism-domain` refuses in
    /// both directions.
    pub fn to_json(&self) -> Result<JsonValue, ShowError> {
        to_json(self)
    }

    // -- internals --------------------------------------------------------

    /// The show changed and has not been saved.
    fn touch(&mut self) {
        self.dirty = true;
    }

    /// The patch changed: everything derived from it has to be rebuilt.
    fn touch_patch(&mut self) {
        self.dirty = true;
        self.patch_revision = self.patch_revision.saturating_add(1);
    }

    /// Rejects a fixture that cannot be patched where it asks to go.
    fn check_fixture(&self, fixture: &Fixture) -> Result<(), ShowError> {
        let Some(fixture_type) = self.fixture_types.get(&fixture.type_id) else {
            return Err(ShowError::UnknownFixtureType(fixture.type_id.clone()));
        };
        if !fixture.universe.is_in_range() {
            return Err(ShowError::UniverseOutOfRange {
                fixture: fixture.id,
                universe: fixture.universe,
            });
        }
        if fixture.last_address(fixture_type.footprint).is_none() {
            return Err(ShowError::AddressOutOfRange {
                fixture: fixture.id,
                address: fixture.address,
                footprint: fixture_type.footprint,
            });
        }
        Ok(())
    }

    /// Rejects a reference to a fixture that is not patched.
    pub(crate) fn require_fixture(&self, id: prism_domain::FixtureId) -> Result<(), ShowError> {
        if self.fixtures.contains_key(&id) {
            Ok(())
        } else {
            Err(ShowError::UnknownFixture(id))
        }
    }

    /// Rejects cues that could not be played or could not be reached.
    fn check_cues(&self, sequence: SequenceId, cues: &[Cue]) -> Result<(), ShowError> {
        let mut seen: Vec<&str> = Vec::with_capacity(cues.len());
        for cue in cues {
            if cue.number.trim().is_empty() {
                return Err(ShowError::EmptyCueNumber);
            }
            if seen.contains(&cue.number.as_str()) {
                return Err(ShowError::DuplicateCueNumber {
                    sequence,
                    number: cue.number.clone(),
                });
            }
            seen.push(&cue.number);
            for part in &cue.parts {
                self.require_fixture(part.fixture)?;
                if let Some(preset) = part.preset_ref
                    && !self.presets.contains_key(&preset)
                {
                    return Err(ShowError::UnknownPreset(preset));
                }
            }
        }
        Ok(())
    }
}

/// Rejects a profile that does not describe an addressable fixture.
fn check_fixture_type(fixture_type: &FixtureType) -> Result<(), ShowError> {
    if fixture_type.id.is_empty() {
        return Err(ShowError::EmptyFixtureTypeId);
    }
    if fixture_type.footprint == 0 {
        return Err(ShowError::EmptyFootprint(fixture_type.id.clone()));
    }
    let mut seen = Vec::with_capacity(fixture_type.attributes.len());
    for attribute in &fixture_type.attributes {
        if seen.contains(&attribute.attribute) {
            return Err(ShowError::DuplicateAttribute {
                type_id: fixture_type.id.clone(),
                attribute: attribute.attribute,
            });
        }
        seen.push(attribute.attribute);
        for offset in [Some(attribute.coarse_offset), attribute.fine_offset]
            .into_iter()
            .flatten()
        {
            if offset >= fixture_type.footprint {
                return Err(ShowError::AttributeOutsideFootprint {
                    type_id: fixture_type.id.clone(),
                    attribute: attribute.attribute,
                    offset,
                    footprint: fixture_type.footprint,
                });
            }
        }
    }
    Ok(())
}

/// A JSON Pointer to one member of one collection.
pub(crate) fn pointer(collection: &str, key: &str) -> String {
    format!("/{collection}/{}", escape(key))
}

/// Escapes a JSON Pointer reference token (RFC 6901 §3).
///
/// A fixture type key is operator data - `generic/rgbw~par` is an unusual name
/// and a legal one - and an unescaped `/` in a pointer is a path separator.
fn escape(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

/// `add` for something new, `replace` for something that was already there.
///
/// RFC 6902 lets `add` overwrite an object member, so one operation would do -
/// but a client that logs its deltas, and a reviewer reading them, learn more
/// from the distinction than the extra branch costs.
fn put(path: String, value: &impl Serialize, existed: bool) -> Result<JsonPatchOp, ShowError> {
    let value = to_json(value)?;
    Ok(if existed {
        JsonPatchOp::Replace { path, value }
    } else {
        JsonPatchOp::Add { path, value }
    })
}

/// Projects anything serialisable into the domain's own JSON value.
fn to_json(value: &impl Serialize) -> Result<JsonValue, ShowError> {
    let value = serde_json::to_value(value)
        .map_err(|error| ShowError::NotRepresentable(error.to_string()))?;
    serde_json::from_value(value).map_err(|error| ShowError::NotRepresentable(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{Show, ShowError};
    use crate::testkit::{cue, dimmer_type, fixture, par_type, sequence};
    use prism_domain::{
        AttributeType, ExecutorId, FixtureId, GroupId, JsonPatchOp, PresetId, SequenceId,
        UniverseId, Vec3,
    };

    #[test]
    fn a_value_that_cannot_be_encoded_is_refused_and_stored_nowhere() {
        // `prism-domain` refuses non-finite floats in both directions (S1), so
        // a NaN that reached the model has no wire form at all. What matters is
        // that the refusal comes before the show is touched: a cue stored
        // behind a returned error is exactly what "a rejection changes nothing"
        // forbids, and the encoding is the last thing that can fail.
        let mut show = Show::new();
        let mut unencodable = par_type();
        unencodable.attributes[0].physical_from = f64::NAN;
        assert!(matches!(
            show.embed_fixture_type(unencodable),
            Err(ShowError::NotRepresentable(_))
        ));
        assert_eq!(show.fixture_types().count(), 0);

        show.embed_fixture_type(par_type()).unwrap();
        show.patch_fixture(fixture(1, "generic.rgbw.par", 1, 1))
            .unwrap();
        show.store_sequence(sequence(1, vec![cue("1", 1, AttributeType::Red, 0)]))
            .unwrap();
        let before = rmp_serde::to_vec_named(&show).unwrap();

        let mut endless = cue("2", 1, AttributeType::Red, 0);
        endless.fade_in = f64::INFINITY;
        assert!(matches!(
            show.store_cue(SequenceId::new(1), endless),
            Err(ShowError::NotRepresentable(_))
        ));
        assert_eq!(rmp_serde::to_vec_named(&show).unwrap(), before);

        let mut nowhere = fixture(2, "generic.rgbw.par", 1, 9);
        nowhere.position = Vec3::new(f64::NAN, 0.0, 0.0);
        assert!(matches!(
            show.patch_fixture(nowhere),
            Err(ShowError::NotRepresentable(_))
        ));
        assert_eq!(rmp_serde::to_vec_named(&show).unwrap(), before);
    }

    #[test]
    fn storing_a_cue_number_twice_replaces_it_and_keeps_the_list_in_order() {
        let mut show = Show::new();
        show.embed_fixture_type(par_type()).unwrap();
        show.patch_fixture(fixture(1, "generic.rgbw.par", 1, 1))
            .unwrap();
        show.store_sequence(sequence(1, vec![cue("1", 1, AttributeType::Red, 1)]))
            .unwrap();

        show.store_cue(SequenceId::new(1), cue("2", 1, AttributeType::Red, 2))
            .unwrap();
        show.store_cue(SequenceId::new(1), cue("1.5", 1, AttributeType::Red, 3))
            .unwrap();
        // The same number again is an edit of that cue, not a second cue with
        // the same name — an operator storing over cue 2 expects one cue 2.
        show.store_cue(SequenceId::new(1), cue("2", 1, AttributeType::Red, 9))
            .unwrap();

        let stored = show.sequence(SequenceId::new(1)).unwrap();
        assert_eq!(
            stored
                .cues
                .iter()
                .map(|cue| cue.number.as_str())
                .collect::<Vec<_>>(),
            ["1", "1.5", "2"]
        );
        assert_eq!(stored.cues[2].parts[0].value, 9);
    }

    #[test]
    fn every_refusal_says_what_was_wrong() {
        // An error an operator cannot read is a mystery on stage.
        for error in [
            ShowError::UnknownFixtureType("x".to_owned()),
            ShowError::UnknownFixture(FixtureId::new(1)),
            ShowError::UnknownGroup(GroupId::new(1)),
            ShowError::UnknownPreset(PresetId::new(1)),
            ShowError::UnknownSequence(SequenceId::new(1)),
            ShowError::UnknownExecutor(ExecutorId::new(1)),
            ShowError::ExecutorHasNoSequence(ExecutorId::new(1)),
            ShowError::EmptyFixtureTypeId,
            ShowError::EmptyFootprint("x".to_owned()),
            ShowError::AttributeOutsideFootprint {
                type_id: "x".to_owned(),
                attribute: AttributeType::Pan,
                offset: 9,
                footprint: 4,
            },
            ShowError::DuplicateAttribute {
                type_id: "x".to_owned(),
                attribute: AttributeType::Pan,
            },
            ShowError::AddressOutOfRange {
                fixture: FixtureId::new(1),
                address: 511,
                footprint: 4,
            },
            ShowError::UniverseOutOfRange {
                fixture: FixtureId::new(1),
                universe: UniverseId::new(65),
            },
            ShowError::TypeChangeBreaksPatch {
                type_id: "x".to_owned(),
                fixture: FixtureId::new(1),
            },
            ShowError::FixtureTypeInUse {
                type_id: "x".to_owned(),
                fixture: FixtureId::new(1),
            },
            ShowError::EmptyCueNumber,
            ShowError::DuplicateCueNumber {
                sequence: SequenceId::new(1),
                number: "1".to_owned(),
            },
            ShowError::ValueOutOfRange(70_000),
            ShowError::NotAShowCommand,
            ShowError::NotRepresentable("NaN".to_owned()),
        ] {
            assert!(!error.to_string().is_empty(), "{error:?}");
        }
        assert_eq!(
            ShowError::AddressOutOfRange {
                fixture: FixtureId::new(3),
                address: 511,
                footprint: 4,
            }
            .to_string(),
            "fixture 3 does not fit: 4 channels from address 511"
        );
    }

    #[test]
    fn an_empty_show_has_nothing_in_it() {
        let show = Show::new();
        assert_eq!(show.fixtures().count(), 0);
        assert_eq!(show.fixture_types().count(), 0);
        assert_eq!(show.groups().count(), 0);
        assert_eq!(show.presets().count(), 0);
        assert_eq!(show.sequences().count(), 0);
        assert_eq!(show.executors().count(), 0);
        assert_eq!(show.universes(), Vec::new());
        assert_eq!(show.patch_revision(), 0);
        assert!(!show.is_dirty());
    }

    #[test]
    fn the_wire_shape_is_one_object_per_collection_keyed_by_identifier() {
        let mut show = Show::new();
        show.embed_fixture_type(par_type()).unwrap();
        show.patch_fixture(fixture(7, "generic.rgbw.par", 1, 1))
            .unwrap();

        let json = serde_json::to_value(&show).unwrap();
        let keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            [
                "executors",
                "fixtureTypes",
                "fixtures",
                "groups",
                "presets",
                "sequences",
            ]
        );
        // Keyed by the number the operator uses, not by position in a list.
        assert_eq!(json["fixtures"]["7"]["name"], "Fixture 7");
        assert_eq!(json["fixtureTypes"]["generic.rgbw.par"]["footprint"], 4);
    }

    #[test]
    fn a_show_survives_both_wire_formats() {
        let mut show = Show::new();
        show.embed_fixture_type(par_type()).unwrap();
        show.patch_fixture(fixture(1, "generic.rgbw.par", 1, 1))
            .unwrap();

        let json = serde_json::to_string(&show).unwrap();
        let from_json: Show = serde_json::from_str(&json).unwrap();
        assert_eq!(
            from_json.fixture(FixtureId::new(1)),
            show.fixture(FixtureId::new(1))
        );

        // S1: MessagePack has to be written with named fields.
        let packed = rmp_serde::to_vec_named(&show).unwrap();
        let from_pack: Show = rmp_serde::from_slice(&packed).unwrap();
        assert_eq!(
            from_pack.fixture(FixtureId::new(1)),
            show.fixture(FixtureId::new(1))
        );
    }

    #[test]
    fn revision_and_dirty_flag_are_not_show_content() {
        let mut show = Show::new();
        show.embed_fixture_type(par_type()).unwrap();
        assert_eq!(show.patch_revision(), 1);
        assert!(show.is_dirty());

        let reloaded: Show = serde_json::from_str(&serde_json::to_string(&show).unwrap()).unwrap();
        assert_eq!(reloaded.patch_revision(), 0);
        assert!(!reloaded.is_dirty());
    }

    #[test]
    fn saving_clears_the_dirty_flag_once() {
        let mut show = Show::new();
        assert!(!show.mark_saved());
        show.embed_fixture_type(par_type()).unwrap();
        assert!(show.mark_saved());
        assert!(!show.is_dirty());
        assert!(!show.mark_saved());
    }

    #[test]
    fn a_fixture_needs_the_profile_to_be_embedded_first() {
        let mut show = Show::new();
        assert_eq!(
            show.patch_fixture(fixture(1, "generic.rgbw.par", 1, 1)),
            Err(ShowError::UnknownFixtureType("generic.rgbw.par".to_owned()))
        );
        assert_eq!(show.fixtures().count(), 0);
        assert_eq!(show.patch_revision(), 0);
    }

    #[test]
    fn patching_names_the_operation_it_performed() {
        let mut show = Show::new();
        show.embed_fixture_type(par_type()).unwrap();

        let ops = show
            .patch_fixture(fixture(3, "generic.rgbw.par", 2, 9))
            .unwrap();
        assert!(matches!(ops.as_slice(), [JsonPatchOp::Add { path, .. }] if path == "/fixtures/3"));

        let ops = show
            .patch_fixture(fixture(3, "generic.rgbw.par", 2, 13))
            .unwrap();
        assert!(
            matches!(ops.as_slice(), [JsonPatchOp::Replace { path, .. }] if path == "/fixtures/3")
        );
        assert_eq!(show.fixture(FixtureId::new(3)).unwrap().address, 13);
        assert_eq!(show.patch_revision(), 3);
    }

    #[test]
    fn a_fixture_that_does_not_fit_is_refused() {
        let mut show = Show::new();
        show.embed_fixture_type(par_type()).unwrap();

        assert_eq!(
            show.patch_fixture(fixture(1, "generic.rgbw.par", 1, 510)),
            Err(ShowError::AddressOutOfRange {
                fixture: FixtureId::new(1),
                address: 510,
                footprint: 4,
            })
        );
        // Exactly at the end of the universe is not out of range.
        show.patch_fixture(fixture(1, "generic.rgbw.par", 1, 509))
            .unwrap();
        assert_eq!(show.fixture(FixtureId::new(1)).unwrap().address, 509);

        assert_eq!(
            show.patch_fixture(fixture(2, "generic.rgbw.par", 1, 0)),
            Err(ShowError::AddressOutOfRange {
                fixture: FixtureId::new(2),
                address: 0,
                footprint: 4,
            })
        );
    }

    #[test]
    fn a_universe_outside_the_supported_range_is_refused() {
        let mut show = Show::new();
        show.embed_fixture_type(par_type()).unwrap();
        assert_eq!(
            show.patch_fixture(fixture(1, "generic.rgbw.par", 65, 1)),
            Err(ShowError::UniverseOutOfRange {
                fixture: FixtureId::new(1),
                universe: UniverseId::new(65),
            })
        );
        assert_eq!(
            show.patch_fixture(fixture(1, "generic.rgbw.par", 0, 1)),
            Err(ShowError::UniverseOutOfRange {
                fixture: FixtureId::new(1),
                universe: UniverseId::new(0),
            })
        );
    }

    #[test]
    fn unpatching_removes_the_fixture_and_nothing_else() {
        let mut show = Show::new();
        show.embed_fixture_type(par_type()).unwrap();
        show.patch_fixture(fixture(1, "generic.rgbw.par", 1, 1))
            .unwrap();
        show.store_group(prism_domain::Group {
            id: GroupId::new(1),
            name: "Wash".to_owned(),
            fixtures: vec![FixtureId::new(1)],
        })
        .unwrap();

        let ops = show.unpatch_fixture(FixtureId::new(1)).unwrap();
        assert_eq!(
            ops,
            vec![JsonPatchOp::Remove {
                path: "/fixtures/1".to_owned()
            }]
        );
        assert!(show.fixture(FixtureId::new(1)).is_none());
        // The group keeps its member: a show outlives the rig it was written on.
        assert_eq!(
            show.group(GroupId::new(1)).unwrap().fixtures,
            vec![FixtureId::new(1)]
        );
        assert_eq!(
            show.unpatch_fixture(FixtureId::new(1)),
            Err(ShowError::UnknownFixture(FixtureId::new(1)))
        );
    }

    #[test]
    fn the_universes_of_a_patch_are_ascending_and_unique() {
        let mut show = Show::new();
        show.embed_fixture_type(par_type()).unwrap();
        for (id, universe) in [(1, 3), (2, 1), (3, 3), (4, 2)] {
            show.patch_fixture(fixture(id, "generic.rgbw.par", universe, 1))
                .unwrap();
        }
        assert_eq!(
            show.universes(),
            vec![UniverseId::new(1), UniverseId::new(2), UniverseId::new(3)]
        );
    }

    #[test]
    fn a_profile_has_to_describe_an_addressable_fixture() {
        let mut show = Show::new();

        let mut nameless = par_type();
        nameless.id = String::new();
        assert_eq!(
            show.embed_fixture_type(nameless),
            Err(ShowError::EmptyFixtureTypeId)
        );

        let mut empty = par_type();
        empty.footprint = 0;
        assert_eq!(
            show.embed_fixture_type(empty),
            Err(ShowError::EmptyFootprint("generic.rgbw.par".to_owned()))
        );

        let mut narrow = par_type();
        narrow.footprint = 2;
        assert_eq!(
            show.embed_fixture_type(narrow),
            Err(ShowError::AttributeOutsideFootprint {
                type_id: "generic.rgbw.par".to_owned(),
                attribute: AttributeType::Blue,
                offset: 2,
                footprint: 2,
            })
        );

        let mut twice = par_type();
        twice.attributes[1].attribute = AttributeType::Red;
        assert_eq!(
            show.embed_fixture_type(twice),
            Err(ShowError::DuplicateAttribute {
                type_id: "generic.rgbw.par".to_owned(),
                attribute: AttributeType::Red,
            })
        );

        assert_eq!(show.fixture_types().count(), 0);
    }

    #[test]
    fn a_sixteen_bit_attribute_needs_its_fine_channel_inside_the_footprint() {
        let mut show = Show::new();
        let mut wide = dimmer_type();
        wide.attributes[0].fine_offset = Some(4);
        assert_eq!(
            show.embed_fixture_type(wide),
            Err(ShowError::AttributeOutsideFootprint {
                type_id: "generic.dimmer".to_owned(),
                attribute: AttributeType::Dimmer,
                offset: 4,
                footprint: 1,
            })
        );
    }

    #[test]
    fn replacing_a_profile_may_not_break_the_patch_standing_on_it() {
        let mut show = Show::new();
        show.embed_fixture_type(par_type()).unwrap();
        show.patch_fixture(fixture(1, "generic.rgbw.par", 1, 510))
            .unwrap_err();
        show.patch_fixture(fixture(1, "generic.rgbw.par", 1, 509))
            .unwrap();

        let mut wider = par_type();
        wider.footprint = 8;
        wider.attributes[3].coarse_offset = 7;
        assert_eq!(
            show.embed_fixture_type(wider),
            Err(ShowError::TypeChangeBreaksPatch {
                type_id: "generic.rgbw.par".to_owned(),
                fixture: FixtureId::new(1),
            })
        );
        assert_eq!(show.fixture_type("generic.rgbw.par").unwrap().footprint, 4);
    }

    #[test]
    fn a_profile_in_use_cannot_be_removed() {
        let mut show = Show::new();
        show.embed_fixture_type(par_type()).unwrap();
        show.patch_fixture(fixture(1, "generic.rgbw.par", 1, 1))
            .unwrap();
        assert_eq!(
            show.remove_fixture_type("generic.rgbw.par"),
            Err(ShowError::FixtureTypeInUse {
                type_id: "generic.rgbw.par".to_owned(),
                fixture: FixtureId::new(1),
            })
        );
        assert_eq!(
            show.remove_fixture_type("nothing"),
            Err(ShowError::UnknownFixtureType("nothing".to_owned()))
        );

        show.unpatch_fixture(FixtureId::new(1)).unwrap();
        assert_eq!(
            show.remove_fixture_type("generic.rgbw.par").unwrap(),
            vec![JsonPatchOp::Remove {
                path: "/fixtureTypes/generic.rgbw.par".to_owned()
            }]
        );
    }

    #[test]
    fn a_pointer_escapes_the_two_characters_that_mean_something() {
        // RFC 6901 §3. A profile key is operator data.
        assert_eq!(
            super::pointer("fixtureTypes", "a/b~c"),
            "/fixtureTypes/a~1b~0c"
        );
    }

    #[test]
    fn the_patched_pairs_are_what_the_engine_asks_for() {
        let mut show = Show::new();
        show.embed_fixture_type(par_type()).unwrap();
        show.patch_fixture(fixture(1, "generic.rgbw.par", 1, 1))
            .unwrap();
        let pairs: Vec<_> = show.patched().collect();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0.id, FixtureId::new(1));
        assert_eq!(pairs[0].1.footprint, 4);
    }

    #[test]
    fn a_fixture_whose_profile_is_missing_is_skipped_rather_than_guessed_at() {
        // Only reachable through a hand-edited file, so it is built that way.
        let mut show = Show::new();
        show.embed_fixture_type(par_type()).unwrap();
        show.patch_fixture(fixture(1, "generic.rgbw.par", 1, 1))
            .unwrap();
        let mut json = serde_json::to_value(&show).unwrap();
        json["fixtureTypes"] = serde_json::json!({});
        let broken: Show = serde_json::from_value(json).unwrap();

        assert_eq!(broken.fixtures().count(), 1);
        assert_eq!(broken.patched().count(), 0);
    }
}
