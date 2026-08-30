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
    Cue, CuePart, CueProperty, CueTrack, CueTracking, CueTrackingMode, CueTrackingRow,
    EXECUTOR_BUTTONS, Executor, ExecutorButtonFunction, ExecutorChange, ExecutorEncoderFunction,
    ExecutorFaderFunction, ExecutorId, Fixture, FixtureType, Group, GroupId, JsonPatchOp,
    JsonValue, PlaybackId, Preset, PresetId, Sequence, SequenceId, TrackedValue, UniverseId,
};
use serde::{Deserialize, Serialize};

use crate::conflict::ShowIssue;
use prism_domain::PatchConflict;

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
///
/// **Not `Eq`**, since S28: [`Self::NegativeTime`] carries the number that was
/// refused, so an operator is told what was wrong with the time they typed
/// rather than that something was. An `f64` has no total equality, and a
/// refusal is compared for equality in tests and nowhere else.
#[derive(Debug, Clone, PartialEq)]
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
    /// An executor has four buttons and this was not one of them — S45.
    NoSuchExecutorButton(u8),
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
    /// A renumber was asked for a number that is already taken. Two fixtures
    /// cannot share one, and replacing the other would delete a light nobody
    /// asked to delete.
    FixtureNumberInUse(prism_domain::FixtureId),
    /// A profile key this desk does not carry. `Command::EmbedFixtureType`
    /// names one of `crate::library`'s, because a client sends a key rather
    /// than a profile (S27).
    UnknownLibraryType(String),
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
    /// A cue number that is not in the sequence.
    UnknownCue {
        /// The sequence.
        sequence: SequenceId,
        /// The number asked for.
        number: String,
    },
    /// A sequence was asked to be created under a number that is taken. Two
    /// sequences cannot share one, and replacing the other would empty a cue
    /// list that may be on stage.
    SequenceNumberInUse(SequenceId),
    /// A fade, a delay or a trigger time below zero. Time runs one way, and a
    /// negative fade is a cue that would have finished before it started.
    NegativeTime(f64),
    /// An absolute attribute value outside `0..=65535`.
    ValueOutOfRange(i32),
    /// A session command reached the show applier. `ARCHITECTURE_SPEC.md` §4.4
    /// commands act on session state, which is S12.
    NotAShowCommand,
    /// A `Copy` or a `Move` naming two different kinds of thing (S40).
    ///
    /// `Copy Cue 2 Group 6` is a line the parser will build and nothing can
    /// carry out, which is the split S26 wrote down: the client decides what was
    /// *asked for* and the daemon decides what is.
    MismatchedObjects {
        /// The source, as an operator would read it back.
        from: String,
        /// The destination.
        to: String,
    },
    /// A `Copy` or a `Move` of something onto itself (S40).
    ///
    /// Refused rather than treated as a no-op: an operator who typed
    /// `Copy Cue 3 Cue 3` meant something else, and silence is the wrong answer
    /// to a line that cannot have been meant.
    SameObject(String),
    /// A `Color` naming something that is not drawn on a strip.
    ///
    /// A colour belongs to a cue list, and an executor's is the list on it — the
    /// other four things `ObjectRef` names have nowhere to show one. Refused
    /// with the word an operator typed rather than ignored, for
    /// [`Self::MismatchedObjects`]'s reason: a line that cannot be carried out
    /// is told so.
    NotColourable(&'static str),
    /// A line that needs the selected cue list, on a desk with none (S40).
    ///
    /// `Store Cue 5` names no sequence and means `Session::selectedSequence`
    /// (§4.1). Nothing selected is a message rather than a silence.
    NoSelectedSequence,
    /// The show could not be projected into JSON. `prism-domain` refuses
    /// non-finite floats in both directions, so this is what a NaN that reached
    /// the model looks like on the way out.
    NotRepresentable(String),
    /// A file command named a path a show cannot live at — S37.
    ///
    /// The extension is the whole check and it is not cosmetic: a `.prism` file
    /// is a SQLite database and a `.json` export is text, so a path with the
    /// wrong one is a command that would either fail obscurely or write one
    /// format under the other's name. An empty path is refused for the reason a
    /// blank surface port is turned into `None`: nothing is not a file.
    NotAShowPath {
        /// What was asked for.
        path: String,
        /// The extension a path here has to have.
        wanted: &'static str,
    },
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
            Self::NoSuchExecutorButton(index) => write!(
                f,
                "an executor has {EXECUTOR_BUTTONS} buttons, numbered from zero, and {index} is not one of them"
            ),
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
            Self::FixtureNumberInUse(id) => {
                write!(f, "fixture {id} is already patched")
            }
            Self::UnknownLibraryType(id) => {
                write!(f, "this desk carries no profile {id:?}")
            }
            Self::FixtureTypeInUse { type_id, fixture } => {
                write!(f, "fixture type {type_id:?} is used by fixture {fixture}")
            }
            Self::EmptyCueNumber => write!(f, "a cue needs a number"),
            Self::DuplicateCueNumber { sequence, number } => {
                write!(f, "sequence {sequence} already has a cue {number:?}")
            }
            Self::UnknownCue { sequence, number } => {
                write!(f, "sequence {sequence} has no cue {number:?}")
            }
            Self::SequenceNumberInUse(id) => {
                write!(f, "sequence {id} already exists")
            }
            Self::NegativeTime(seconds) => {
                write!(f, "{seconds} seconds is not a time a cue can have")
            }
            Self::ValueOutOfRange(value) => {
                write!(f, "attribute value {value} is outside 0..=65535")
            }
            Self::NotAShowCommand => write!(f, "this is a session command, not a show command"),
            Self::MismatchedObjects { from, to } => {
                write!(f, "{from} and {to} are not the same kind of thing")
            }
            Self::SameObject(what) => write!(f, "{what} is already where it is"),
            Self::NotColourable(noun) => write!(
                f,
                "a {noun} has no colour: a colour belongs to a sequence, a preset, or to \n                 the sequence on an executor"
            ),
            Self::NoSelectedSequence => {
                write!(
                    f,
                    "no cue list is selected: say which one, or select one first"
                )
            }
            Self::NotRepresentable(reason) => write!(f, "the show cannot be encoded: {reason}"),
            Self::NotAShowPath { path, wanted } => {
                write!(f, "{path:?} is not a {wanted} file")
            }
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

/// The intensity the desk supplies for a fixture whose profile has none — S43.
///
/// **It has no channel, and the offsets say nothing about one.** Nothing derives
/// a DMX address from an `AttributeDef` reached through
/// [`Show::attribute_def`]; addresses come from `FixtureType::attributes`, which
/// this is deliberately not in, and `prism_engine::ChannelPlan` builds from that
/// list alone. What this definition is for is the three questions the programmer
/// asks — may a value exist, where does a relative move start, and which bank is
/// it on — and the answers are yes, nought, and intensity.
///
/// Nought is the answer that matters: it is what makes a rig of colour-only
/// fixtures dark at home now that colour rests open (punch-list B1).
static SOFTWARE_DIMMER: prism_domain::AttributeDef = prism_domain::AttributeDef {
    attribute: prism_domain::AttributeType::Dimmer,
    feature_group: prism_domain::FeatureGroup::Dimmer,
    coarse_offset: 0,
    fine_offset: None,
    default_value: 0,
    merge_mode: prism_domain::MergeMode::Htp,
    invert: false,
    physical_from: 0.0,
    physical_to: 100.0,
};

impl Show {
    /// An empty show: no profiles, no patch, nothing stored.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The show a `.prism` file's rows add up to.
    ///
    /// Deliberately **not** built by replaying the validated edits above, and
    /// the reason is a show that is legal to hold and not legal to store: S11
    /// decided that a dangling reference is reported rather than refused, so a
    /// cue list naming a fixture somebody unpatched afterwards is an ordinary
    /// show that [`Show::store_sequence`] would nonetheless turn down. A loader
    /// built out of the edit operations would refuse to open the file it had
    /// itself written. What the file is checked for instead is what only the
    /// file can be wrong about — a row that does not decode, or one filed under
    /// a key that is not its own — and [`Show::issues`] reports the rest to the
    /// operator exactly as it does for a show that was edited into that state
    /// in front of them.
    ///
    /// The caller has checked that every key is the identifier its value
    /// carries; [`crate::ShowStore`] is the only one, and it is what the check
    /// belongs to because it is the only layer that knows a key can come from
    /// somewhere other than the value.
    pub(crate) fn from_parts(
        fixture_types: BTreeMap<String, FixtureType>,
        fixtures: BTreeMap<prism_domain::FixtureId, Fixture>,
        groups: BTreeMap<GroupId, Group>,
        presets: BTreeMap<PresetId, Preset>,
        sequences: BTreeMap<SequenceId, Sequence>,
        executors: BTreeMap<ExecutorId, Executor>,
    ) -> Self {
        Self {
            fixture_types,
            fixtures,
            groups,
            presets,
            sequences,
            executors,
            // Neither is show content (see the module documentation): a show
            // that has just been read has nothing unsaved in it, and the
            // revision counts this run's patch changes.
            patch_revision: 0,
            dirty: false,
        }
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

    /// One cue of one sequence, by the number an operator typed.
    ///
    /// Trimmed on the way in, so `" 2 "` and `"2"` are the same cue — the rule
    /// [`Programmer::cue`](crate::Programmer::cue) and
    /// [`Self::set_cue_property`] already apply, in one place so that S39's
    /// `EditCue` cannot read a number the store would file differently.
    #[must_use]
    pub fn cue(&self, sequence: SequenceId, number: &str) -> Option<&Cue> {
        let wanted = number.trim();
        self.sequences
            .get(&sequence)?
            .cues
            .iter()
            .find(|cue| cue.number == wanted)
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

    /// The definition of one attribute of one patched fixture.
    ///
    /// The programmer (S13) asks this twice for every value it writes: for the
    /// home value a relative move starts from, and for the feature group the
    /// attribute is filed under — which is the *profile's* answer rather than
    /// the attribute name's, the distinction S6 had to make before the masters
    /// could be written. `None` means the fixture is not patched, or its
    /// profile has no such attribute, which are the same answer to "may this
    /// value exist": no.
    ///
    /// **A fixture whose intensity the desk supplies has a `Dimmer` here that
    /// its profile has not** — S43. It is [`SOFTWARE_DIMMER`], and it is the
    /// same answer the merge gives (`prism_engine::MergePlan::build` adds the
    /// matching slot): a value on it may exist, it rests at nought, and it is
    /// filed under the intensity bank, so the programmer, the encoders and the
    /// command line all reach it exactly as they reach a real one.
    #[must_use]
    pub fn attribute_def(
        &self,
        fixture: prism_domain::FixtureId,
        attribute: prism_domain::AttributeType,
    ) -> Option<&prism_domain::AttributeDef> {
        let fixture = self.fixtures.get(&fixture)?;
        let fixture_type = self.fixture_types.get(&fixture.type_id)?;
        if attribute == prism_domain::AttributeType::Dimmer
            && fixture.has_software_dimmer(fixture_type)
        {
            return Some(&SOFTWARE_DIMMER);
        }
        fixture_type
            .attributes
            .iter()
            .find(|def| def.attribute == attribute)
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

    /// Records that the show holds something the file does not — S37.
    ///
    /// Every ordinary edit lights the lamp on its own way through
    /// [`Show::apply`]. This exists for the one change that is not an edit:
    /// `Command::ImportShow` **replaces** the show with a document read out of a
    /// JSON export, which arrives clean because it has just been deserialised.
    /// An import that left the lamp dark would be telling an operator their
    /// `.prism` file already held what they had just read in — and it does not,
    /// deliberately, because an import somebody did not mean to do must be one
    /// they can walk away from.
    pub const fn mark_dirty(&mut self) {
        self.dirty = true;
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

    /// What patching a fixture at an address *would* do, without doing it.
    ///
    /// The answer to `Query::PatchPreview`, and the reason S27 could show an
    /// address conflict **before** it was committed rather than warning about
    /// one afterwards. See [`crate::conflict`]: an overlap is legal, so the
    /// answer distinguishes *would be refused* from *would overlap*.
    #[must_use]
    pub fn preview_patch(
        &self,
        id: prism_domain::FixtureId,
        type_id: &str,
        universe: UniverseId,
        address: u16,
    ) -> prism_domain::PatchPreview {
        crate::conflict::preview(self, id, type_id, universe, address)
    }

    /// Whether [`Self::patch_fixture`] would accept these five fields.
    ///
    /// Writes nothing. The validation is the same code path the edit runs, so a
    /// preview and the patch that follows it cannot disagree.
    ///
    /// # Errors
    ///
    /// Whatever [`Self::patch_fixture`] would answer with.
    pub(crate) fn check_patch(
        &self,
        id: prism_domain::FixtureId,
        type_id: &str,
        universe: UniverseId,
        address: u16,
    ) -> Result<(), ShowError> {
        self.check_fixture(&Fixture {
            software_dimmer: true,
            id,
            name: String::new(),
            type_id: type_id.to_owned(),
            universe,
            address,
            position: prism_domain::Vec3::ZERO,
            rotation: prism_domain::Vec3::ZERO,
            invert_pan: false,
            invert_tilt: false,
        })
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

    /// Gives a patched fixture a different number.
    ///
    /// One operation rather than an unpatch and a patch, because the number is
    /// the key: doing it in two steps would leave the rig without that fixture
    /// in between, and a refusal on the second step would leave it deleted.
    /// Everything else about the fixture travels with it — the name, the type,
    /// the address, the position, the rotation and the inverts — so a renumber
    /// is exactly a renumber.
    ///
    /// Deliberately does **not** follow the number into groups, presets or cues.
    /// Those keep pointing at the old number, which [`Show::issues`] then reports
    /// as dangling: the alternative is rewriting an operator's stored looks
    /// underneath them, and S11 settled that a show outlives its rig rather than
    /// the other way round.
    ///
    /// # Errors
    ///
    /// [`ShowError::UnknownFixture`] if `from` is not patched, or
    /// [`ShowError::FixtureNumberInUse`] if `to` already is — two fixtures
    /// cannot share a number, and replacing the other one would delete a light
    /// nobody asked to delete.
    pub fn renumber_fixture(
        &mut self,
        from: prism_domain::FixtureId,
        to: prism_domain::FixtureId,
    ) -> Result<Vec<JsonPatchOp>, ShowError> {
        let Some(existing) = self.fixtures.get(&from) else {
            return Err(ShowError::UnknownFixture(from));
        };
        if from == to {
            // Not an error and not a change: an operator who typed the number
            // that was already there has asked for nothing.
            return Ok(Vec::new());
        }
        if self.fixtures.contains_key(&to) {
            return Err(ShowError::FixtureNumberInUse(to));
        }
        let moved = Fixture {
            id: to,
            ..existing.clone()
        };
        // Built before anything is written, so a projection failure leaves the
        // show exactly as it was — S11's rule, which is why `put` comes first.
        let op = put(pointer(FIXTURES, &to.to_string()), &moved, false)?;
        self.fixtures.remove(&from);
        self.fixtures.insert(to, moved);
        self.touch_patch();
        Ok(vec![
            JsonPatchOp::Remove {
                path: pointer(FIXTURES, &from.to_string()),
            },
            op,
        ])
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
        // Everything fallible first, then nothing is written until all of it
        // has succeeded — S11's rule, and the reason the sequences are built
        // beside the old ones rather than edited in place.
        let relinked = self.relink(&preset)?;
        let mut ops = vec![op];
        for (op, sequence) in relinked {
            ops.push(op);
            self.sequences.insert(sequence.id, sequence);
        }
        self.presets.insert(preset.id, preset);
        self.touch();
        Ok(ops)
    }

    /// Every cue part linked to this preset, given the preset's new values.
    ///
    /// **This is what makes a preset link a link.** `prism_domain::preset` says
    /// it in the first paragraph of its module documentation — *a cue part that
    /// carries a `presetRef` follows later edits of the preset, which is what
    /// makes "change the blue everywhere" a one-touch operation on a real
    /// console* — and until S28 nothing did it: a cue stored the preset's value
    /// and the number beside it, and editing the preset moved neither.
    ///
    /// It is done here, in the show, rather than in the engine, because the
    /// engine plays `CuePart::value` and must not resolve anything per tick
    /// (§3.1). So the value in the cue is always the value that will be output,
    /// and the link is what keeps it current.
    ///
    /// A part linked to a preset that no longer carries that fixture and
    /// attribute keeps **both** its value and its link. Dropping the value would
    /// change light nobody asked to change, and dropping the link would mean a
    /// preset that regained the value could never reach the cue again —
    /// `Show::remove_preset` already takes the same view of a preset that has
    /// gone altogether.
    fn relink(&self, preset: &Preset) -> Result<Vec<(JsonPatchOp, Sequence)>, ShowError> {
        let mut updated = Vec::new();
        for sequence in self.sequences.values() {
            let mut next = sequence.clone();
            let mut moved = false;
            for cue in &mut next.cues {
                for part in &mut cue.parts {
                    if part.preset_ref != Some(preset.id) {
                        continue;
                    }
                    let Some(value) = preset
                        .values
                        .iter()
                        .find(|value| {
                            value.fixture == part.fixture && value.attribute == part.attribute
                        })
                        .map(|value| value.value)
                    else {
                        continue;
                    };
                    if part.value != value {
                        part.value = value;
                        moved = true;
                    }
                }
            }
            if moved {
                let path = pointer(SEQUENCES, &next.id.to_string());
                updated.push((put(path, &next, true)?, next));
            }
        }
        Ok(updated)
    }

    /// Which sequences a preset's values reach, so a caller can reload exactly
    /// those.
    ///
    /// Reported rather than inferred from [`Self::store_preset`]'s operations,
    /// because the caller needs it **before** the store in order to name the
    /// engine effect, and because an operation list is a description of a
    /// document rather than of what has to be reloaded.
    #[must_use]
    pub fn sequences_using_preset(&self, preset: PresetId) -> Vec<SequenceId> {
        self.sequences
            .values()
            .filter(|sequence| {
                sequence
                    .cues
                    .iter()
                    .any(|cue| cue.parts.iter().any(|part| part.preset_ref == Some(preset)))
            })
            .map(|sequence| sequence.id)
            .collect()
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

    /// Creates an empty sequence.
    ///
    /// **Refused when the number is taken**, which is the whole difference
    /// between this and [`Self::store_sequence`]: this is what an interface
    /// sends, and a *create* that silently replaced a cue list would empty a
    /// playback that may be running. `Command::CreateSequence` explains why it
    /// is not S39's `StoreSequence`, which is a different act with a mode on it.
    ///
    /// # Errors
    ///
    /// [`ShowError::SequenceNumberInUse`] if there is already a sequence there.
    pub fn create_sequence(
        &mut self,
        id: SequenceId,
        name: &str,
    ) -> Result<Vec<JsonPatchOp>, ShowError> {
        if self.sequences.contains_key(&id) {
            return Err(ShowError::SequenceNumberInUse(id));
        }
        self.store_sequence(Sequence {
            id,
            name: name.to_owned(),
            color: None,
            cues: Vec::new(),
            looping: false,
            master_level: u16::MAX,
            speed: prism_domain::SPEED_UNITY,
            is_active: false,
            current_cue_index: None,
        })
    }

    /// Changes one field of one cue.
    ///
    /// The delta carries the whole sequence, for [`Self::store_cue`]'s reason: a
    /// cue list is small, and a renumber moves the cue within it.
    ///
    /// Answers with **no operations at all** when the field already holds that
    /// value. A `ShowPatch` describing a document that did not move is a
    /// broadcast to every client that says nothing, and — as
    /// `Command::RenumberFixture` found in S27 — it is also an Oops step an
    /// operator would press and watch do nothing.
    ///
    /// # Errors
    ///
    /// [`ShowError::UnknownSequence`], [`ShowError::UnknownCue`],
    /// [`ShowError::EmptyCueNumber`], [`ShowError::DuplicateCueNumber`] for a
    /// renumber onto a cue that exists, or [`ShowError::NegativeTime`].
    pub fn set_cue_property(
        &mut self,
        sequence_id: SequenceId,
        cue_number: &str,
        property: &CueProperty,
    ) -> Result<Vec<JsonPatchOp>, ShowError> {
        let Some(sequence) = self.sequences.get(&sequence_id) else {
            return Err(ShowError::UnknownSequence(sequence_id));
        };
        // The operator typed the number, so `" 2 "` and `"2"` are the same cue —
        // the rule `Programmer::cue` already applies on the way in.
        let wanted = cue_number.trim();
        let Some(index) = sequence.cues.iter().position(|cue| cue.number == wanted) else {
            return Err(ShowError::UnknownCue {
                sequence: sequence_id,
                number: wanted.to_owned(),
            });
        };

        let mut next = sequence.clone();
        let moved = apply_cue_property(&mut next, index, property)?;
        if !moved {
            return Ok(Vec::new());
        }
        next.cues
            .sort_by(|left, right| Cue::compare_numbers(&left.number, &right.number));
        let op = put(pointer(SEQUENCES, &sequence_id.to_string()), &next, true)?;
        self.sequences.insert(sequence_id, next);
        self.touch();
        Ok(vec![op])
    }

    /// What every cue of one list inherits, in playback order — **S48**.
    ///
    /// The answer to `prism_domain::Query::CueTracking`, computed here so that
    /// no client computes it. Each row carries the attributes the list holds at
    /// that cue which the cue does **not** name, and whether the cue asserts
    /// everything — which is the same fact said twice, because a cue that
    /// inherits nothing is a blocking cue and there is no third way to be one.
    ///
    /// An unknown sequence answers with **no rows** rather than an error: a
    /// window asking about a list somebody has just deleted is a race and not a
    /// mistake, and `Query` has no refusal shape by design (`docs/IPC_PROTOCOL.md`
    /// section 5.2).
    #[must_use]
    pub fn cue_tracking(&self, sequence_id: SequenceId) -> Vec<CueTrackingRow> {
        let Some(sequence) = self.sequences.get(&sequence_id) else {
            return Vec::new();
        };
        let order = sequence.ordered_cues();
        let mut track = CueTrack::new();
        let mut changes = Vec::new();
        order
            .into_iter()
            .map(|cue| {
                track.enter(cue, &mut changes);
                let inherited: Vec<TrackedValue> = track
                    .inherited(cue)
                    .into_iter()
                    .map(|((fixture, attribute), value)| TrackedValue {
                        fixture,
                        attribute,
                        value,
                    })
                    .collect();
                CueTrackingRow {
                    number: cue.number.clone(),
                    blocks: inherited.is_empty(),
                    inherited,
                }
            })
            .collect()
    }

    /// Says what a whole cue does about tracking — **S48**.
    ///
    /// Two of the three modes rewrite every part's
    /// [`prism_domain::CueTracking`]. The third, [`CueTrackingMode::Block`],
    /// **writes the values the cue inherits into it**, so that nothing above it
    /// reaches past it and a list can be cut into sections an operator can
    /// rehearse from. That is an edit and not a mode, for the reason
    /// [`CueTrackingMode`] gives in full: a flag honoured at playback time would
    /// still be computed from the cues above, so editing cue 2 would go on
    /// changing what a blocking cue 5 puts out — which is the one thing blocking
    /// is asked for to stop.
    ///
    /// The values a block writes are the daemon's own, folded out of the cues
    /// above by `prism_domain::CueTrack`, and they carry **no preset link**: an
    /// inherited value is the result of somebody else's edit, and copying the
    /// link would make a later `StorePreset` rewrite a cue nobody stored into.
    /// A cue-only part **becomes a tracking one**, because a value that is taken
    /// back at the end is not an assertion and a blocking cue asserts
    /// everything.
    ///
    /// Answers with **no operations at all** when nothing moved -
    /// [`Self::set_cue_property`]'s rule, and for its reason.
    ///
    /// # Errors
    ///
    /// [`ShowError::UnknownSequence`] or [`ShowError::UnknownCue`].
    pub fn set_cue_tracking(
        &mut self,
        sequence_id: SequenceId,
        cue_number: &str,
        tracking: CueTrackingMode,
    ) -> Result<Vec<JsonPatchOp>, ShowError> {
        let Some(sequence) = self.sequences.get(&sequence_id) else {
            return Err(ShowError::UnknownSequence(sequence_id));
        };
        let wanted = cue_number.trim();
        let Some(index) = sequence
            .cues
            .iter()
            .position(|cue| cue.number.trim() == wanted)
        else {
            return Err(ShowError::UnknownCue {
                sequence: sequence_id,
                number: wanted.to_owned(),
            });
        };
        // What a cue inherits is decided by the cues that play **before** it, so
        // the walk is over `ordered_cues` — the playback order — and not over
        // the file. The two differ the moment a cue is inserted between two
        // others, which is the entire reason cue numbers are decimal strings.
        let inherited: Vec<(prism_domain::CueKey, u16)> = match tracking {
            CueTrackingMode::Block => sequence
                .tracking()
                .into_iter()
                .zip(sequence.ordered_cues())
                .filter(|(_, cue)| cue.number.trim() == wanted)
                .take(1)
                .flat_map(|(track, cue)| track.inherited(cue))
                .collect(),
            CueTrackingMode::Track | CueTrackingMode::CueOnly => Vec::new(),
        };

        let mut next = sequence.clone();
        let cue = &mut next.cues[index];
        let want = match tracking {
            CueTrackingMode::CueOnly => CueTracking::CueOnly,
            CueTrackingMode::Track | CueTrackingMode::Block => CueTracking::Track,
        };
        let mut moved = false;
        for part in &mut cue.parts {
            moved |= replace(&mut part.tracking, want);
        }
        for ((fixture, attribute), value) in inherited {
            cue.parts.push(CuePart {
                fixture,
                attribute,
                value,
                preset_ref: None,
                tracking: CueTracking::Track,
            });
            moved = true;
        }
        if !moved {
            return Ok(Vec::new());
        }
        let op = put(pointer(SEQUENCES, &sequence_id.to_string()), &next, true)?;
        self.sequences.insert(sequence_id, next);
        self.touch();
        Ok(vec![op])
    }

    /// Takes one cue out of a sequence.
    ///
    /// The cues after it **keep their numbers**. A cue number is what an
    /// operator has written on a running order and what a Goto names, so
    /// closing the gap would silently move every cue after the deleted one.
    ///
    /// # Errors
    ///
    /// [`ShowError::UnknownSequence`] or [`ShowError::UnknownCue`].
    pub fn remove_cue(
        &mut self,
        sequence_id: SequenceId,
        cue_number: &str,
    ) -> Result<Vec<JsonPatchOp>, ShowError> {
        let Some(sequence) = self.sequences.get(&sequence_id) else {
            return Err(ShowError::UnknownSequence(sequence_id));
        };
        let wanted = cue_number.trim();
        let Some(index) = sequence.cues.iter().position(|cue| cue.number == wanted) else {
            return Err(ShowError::UnknownCue {
                sequence: sequence_id,
                number: wanted.to_owned(),
            });
        };
        let mut next = sequence.clone();
        next.cues.remove(index);
        let op = put(pointer(SEQUENCES, &sequence_id.to_string()), &next, true)?;
        self.sequences.insert(sequence_id, next);
        self.touch();
        Ok(vec![op])
    }

    /// Puts a sequence on an executor, or takes one off.
    ///
    /// An empty slot gains an executor with **the desk's defaults**, and those
    /// defaults are here rather than in a client for the reason
    /// `Command::PatchFixture` carries no channels: what a fader and four
    /// buttons do is show content, and a client that chose it would be authoring
    /// the show for the daemon to accept. They are the three functions the
    /// protocol can actually press (S22, S26) in the order a console has them,
    /// and a fourth button left [`ExecutorButtonFunction::Empty`] — because the
    /// five functions that have no command yet are **S34**'s, and a default that
    /// put one there would ship a button the interface has to draw disabled.
    ///
    /// The master is not among them since S45: it belongs to the cue list, and
    /// a list nobody has faded rests at full for the reason a new executor used
    /// to — a Go that produced no light would be indistinguishable from a desk
    /// that is broken.
    ///
    /// Answers with no operations when that executor already plays that
    /// sequence.
    ///
    /// # Errors
    ///
    /// [`ShowError::UnknownSequence`] if the sequence does not exist.
    pub fn assign_executor(
        &mut self,
        id: ExecutorId,
        sequence_id: Option<SequenceId>,
    ) -> Result<Vec<JsonPatchOp>, ShowError> {
        if let Some(sequence) = sequence_id
            && !self.sequences.contains_key(&sequence)
        {
            return Err(ShowError::UnknownSequence(sequence));
        }
        let existed = self.executors.contains_key(&id);
        let next = match self.executors.get(&id) {
            Some(executor) if executor.sequence_id == sequence_id => return Ok(Vec::new()),
            // An existing slot keeps everything else it has: its master, its
            // button functions and its encoder. Taking the sequence off is not
            // the same act as emptying the slot, and S34 and S38 are what give
            // an operator a reason to have set those in the first place.
            Some(executor) => Executor {
                sequence_id,
                ..executor.clone()
            },
            None => match sequence_id {
                Some(_) => default_executor(id, sequence_id),
                // There is nothing to take off a slot that has nothing on it,
                // and making an empty executor in order to clear it would leave
                // the show a row longer than it started.
                None => return Ok(Vec::new()),
            },
        };
        let op = put(pointer(EXECUTORS, &id.to_string()), &next, existed)?;
        self.executors.insert(id, next);
        self.touch();
        Ok(vec![op])
    }

    /// Empties an executor slot.
    ///
    /// The inverse of the [`Self::assign_executor`] that created one, and the
    /// only way a slot leaves the show — which is why it exists: an Oops over a
    /// newly assigned executor has to be able to put the grid back exactly as it
    /// was, and a slot left behind with no sequence on it is a row the show did
    /// not have before.
    ///
    /// # Errors
    ///
    /// [`ShowError::UnknownExecutor`] if the slot is already empty.
    pub fn remove_executor(&mut self, id: ExecutorId) -> Result<Vec<JsonPatchOp>, ShowError> {
        if self.executors.remove(&id).is_none() {
            return Err(ShowError::UnknownExecutor(id));
        }
        self.touch();
        Ok(vec![JsonPatchOp::Remove {
            path: pointer(EXECUTORS, &id.to_string()),
        }])
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

    /// Moves a cue list's master level.
    ///
    /// **It was the executor's until S45** and the move is punch-list entry
    /// B18: one number per list, so two executors whose faders are both
    /// `Master` are two handles on it rather than two opinions about it. Which
    /// number a fader moves is `Show::apply`'s answer, from the executor's own
    /// `fader_function`.
    ///
    /// # Errors
    ///
    /// [`ShowError::UnknownSequence`] if there is no such cue list.
    pub fn set_sequence_master(
        &mut self,
        id: SequenceId,
        level: u16,
    ) -> Result<Vec<JsonPatchOp>, ShowError> {
        let Some(sequence) = self.sequences.get_mut(&id) else {
            return Err(ShowError::UnknownSequence(id));
        };
        sequence.master_level = level;
        self.touch();
        Ok(vec![JsonPatchOp::Replace {
            path: format!("{}/masterLevel", pointer(SEQUENCES, &id.to_string())),
            value: JsonValue::Int(i64::from(level)),
        }])
    }

    /// Moves a cue list's speed master — `docs/DMX_MERGE.md` §4 item 3.
    ///
    /// The same move as [`Self::set_sequence_master`] and the same reason: a
    /// rate an operator set is part of the show, and two faders set to `Speed`
    /// on one list are two handles on one rate.
    ///
    /// # Errors
    ///
    /// [`ShowError::UnknownSequence`] if there is no such cue list.
    pub fn set_sequence_speed(
        &mut self,
        id: SequenceId,
        speed: u16,
    ) -> Result<Vec<JsonPatchOp>, ShowError> {
        let Some(sequence) = self.sequences.get_mut(&id) else {
            return Err(ShowError::UnknownSequence(id));
        };
        sequence.speed = speed;
        self.touch();
        Ok(vec![JsonPatchOp::Replace {
            path: format!("{}/speed", pointer(SEQUENCES, &id.to_string())),
            value: JsonValue::Int(i64::from(speed)),
        }])
    }

    /// Says what one of an executor's controls does — **S45**, punch-list entry
    /// B15.
    ///
    /// One control at a time (`prism_domain::ExecutorChange`). Answers with no
    /// operations when the control already does that, so a chooser that sent the
    /// function already in force broadcasts nothing.
    ///
    /// # Errors
    ///
    /// [`ShowError::UnknownExecutor`] if the slot is empty — a function on a
    /// fader that is not there is a line an operator can act on, where silence
    /// is not. [`ShowError::NoSuchExecutorButton`] for an index past the four a
    /// strip has: refused rather than clamped, because a desk that quietly
    /// assigned a fifth key would be answering a question nobody asked.
    pub fn configure_executor(
        &mut self,
        id: ExecutorId,
        change: &ExecutorChange,
    ) -> Result<Vec<JsonPatchOp>, ShowError> {
        let Some(executor) = self.executors.get_mut(&id) else {
            return Err(ShowError::UnknownExecutor(id));
        };
        let path = pointer(EXECUTORS, &id.to_string());
        let op = match change {
            ExecutorChange::Fader { function } => {
                if executor.fader_function == *function {
                    return Ok(Vec::new());
                }
                executor.fader_function = *function;
                JsonPatchOp::Replace {
                    path: format!("{path}/faderFunction"),
                    value: to_json(function)?,
                }
            }
            ExecutorChange::Encoder { function } => {
                if executor.encoder_function == *function {
                    return Ok(Vec::new());
                }
                executor.encoder_function = *function;
                JsonPatchOp::Replace {
                    path: format!("{path}/encoderFunction"),
                    value: to_json(function)?,
                }
            }
            ExecutorChange::Button { index, function } => {
                if *index >= EXECUTOR_BUTTONS {
                    return Err(ShowError::NoSuchExecutorButton(*index));
                }
                // **The row is filled in rather than extended.** A slot made
                // before S38's fourth default, or one an older file wrote with
                // two entries, has a short list — and assigning the fourth key
                // has to mean the fourth key rather than the third. The gap is
                // filled with `Empty`, which is what those positions already do.
                let slot = usize::from(*index);
                while executor.button_functions.len() <= slot {
                    executor
                        .button_functions
                        .push(ExecutorButtonFunction::Empty);
                }
                if executor.button_functions[slot] == *function {
                    return Ok(Vec::new());
                }
                executor.button_functions[slot] = function.clone();
                // The **whole row**, not the one entry: a list that had to grow
                // to reach the index would leave a client's mirror with holes in
                // it if only the last position were sent.
                JsonPatchOp::Replace {
                    path: format!("{path}/buttonFunctions"),
                    value: to_json(&executor.button_functions)?,
                }
            }
        };
        self.touch();
        Ok(vec![op])
    }

    /// Records what the engine reports about a running playback.
    ///
    /// This travels as `Delta::PlaybackState` rather than as a show patch: it
    /// is playback state the engine owns, and the protocol gives it its own
    /// delta so a client does not have to diff the show to draw a moving
    /// executor bar.
    ///
    /// **It is written onto the cue list, and there is one row per list**
    /// (S45). An executor draws its own row by reading through to the list
    /// standing on it, which is what makes the second executor of one sequence
    /// say the same thing as the first — punch-list entry B18. `crate::mirror`
    /// writes it into the same place, from the same delta, which is what keeps a
    /// client's document and the daemon's agreeing.
    ///
    /// Returns whether anything actually changed.
    ///
    /// # Errors
    ///
    /// [`ShowError::UnknownSequence`] if the playback is not in this show —
    /// which is ordinary rather than a fault: the tick is one merge body behind
    /// for a poll after a rebuild.
    pub fn record_playback_state(
        &mut self,
        playback: PlaybackId,
        is_active: bool,
        cue_index: Option<u32>,
    ) -> Result<bool, ShowError> {
        let sequence_id = playback.sequence();
        let Some(sequence) = self.sequences.get_mut(&sequence_id) else {
            return Err(ShowError::UnknownSequence(sequence_id));
        };
        let changed = sequence.is_active != is_active || sequence.current_cue_index != cue_index;
        sequence.is_active = is_active;
        sequence.current_cue_index = cue_index;
        // Deliberately not marked dirty: a playback running is not an unsaved
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

    /// The group pool, for writing. `crate::objects` is the other author.
    pub(crate) const fn groups_mut(&mut self) -> &mut BTreeMap<GroupId, Group> {
        &mut self.groups
    }

    /// The preset pools, for writing.
    pub(crate) const fn presets_mut(&mut self) -> &mut BTreeMap<PresetId, Preset> {
        &mut self.presets
    }

    /// The sequence pool, for writing.
    pub(crate) const fn sequences_mut(&mut self) -> &mut BTreeMap<SequenceId, Sequence> {
        &mut self.sequences
    }

    /// [`Self::touch`], reachable from `crate::objects`.
    pub(crate) const fn mark(&mut self) {
        self.dirty = true;
    }

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

/// One field of a cue, written into a copy of the sequence.
///
/// Answers whether anything moved. Every refusal happens before the copy is
/// handed back, so a rejected edit leaves the sequence untouched.
fn apply_cue_property(
    sequence: &mut Sequence,
    index: usize,
    property: &CueProperty,
) -> Result<bool, ShowError> {
    match property {
        CueProperty::FadeIn { seconds } => {
            Ok(replace(&mut sequence.cues[index].fade_in, time(*seconds)?))
        }
        CueProperty::FadeOut { seconds } => {
            Ok(replace(&mut sequence.cues[index].fade_out, time(*seconds)?))
        }
        CueProperty::Delay { seconds } => {
            Ok(replace(&mut sequence.cues[index].delay, time(*seconds)?))
        }
        CueProperty::Trigger {
            trigger,
            trigger_time,
        } => {
            let checked = match trigger_time {
                Some(seconds) => Some(time(*seconds)?),
                None => None,
            };
            let cue = &mut sequence.cues[index];
            let moved = replace(&mut cue.trigger, *trigger);
            Ok(replace(&mut cue.trigger_time, checked) || moved)
        }
    }
}

/// Writes a value and answers whether it was different.
fn replace<T: PartialEq>(target: &mut T, value: T) -> bool {
    if *target == value {
        return false;
    }
    *target = value;
    true
}

/// A time a cue can have.
///
/// Non-finite values never reach here — `prism_domain::finite` refuses them at
/// the decoder — so the only thing left to check is the sign, and a fade that
/// ran backwards would be a cue that finished before it started.
fn time(seconds: f64) -> Result<f64, ShowError> {
    if seconds < 0.0 {
        return Err(ShowError::NegativeTime(seconds));
    }
    Ok(seconds)
}

/// The executor an empty slot gains when a sequence is put on it.
///
/// See [`Show::assign_executor`] for why the defaults are the daemon's.
fn default_executor(id: ExecutorId, sequence_id: Option<SequenceId>) -> Executor {
    Executor {
        id,
        sequence_id,
        fader_function: ExecutorFaderFunction::Master,
        // Rec, Solo, Mute, Select — the hardware order (`docs/MCU_MAPPING.md`).
        button_functions: vec![
            ExecutorButtonFunction::GoForward,
            ExecutorButtonFunction::GoBack,
            ExecutorButtonFunction::Off,
            ExecutorButtonFunction::Empty,
        ],
        encoder_function: ExecutorEncoderFunction::Empty,
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
pub(crate) fn put(
    path: String,
    value: &impl Serialize,
    existed: bool,
) -> Result<JsonPatchOp, ShowError> {
    let value = to_json(value)?;
    Ok(if existed {
        JsonPatchOp::Replace { path, value }
    } else {
        JsonPatchOp::Add { path, value }
    })
}

/// Projects anything serialisable into the domain's own JSON value.
fn to_json(value: &impl Serialize) -> Result<JsonValue, ShowError> {
    project(value).map_err(ShowError::NotRepresentable)
}

/// The projection itself, without an error type attached to it.
///
/// Shared with [`crate::SessionState`], which has to make the same trip and
/// reports it as its own error. Fallible for the reason S1 recorded: a
/// non-finite float has no wire form, and `prism-domain` refuses one in both
/// directions.
pub(crate) fn project(value: &impl Serialize) -> Result<JsonValue, String> {
    let value = serde_json::to_value(value).map_err(|error| error.to_string())?;
    serde_json::from_value(value).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{Show, ShowError};
    use crate::testkit::{cue, dimmer_type, fixture, par_type, sequence};
    use prism_domain::{
        AttributeType, ExecutorId, FixtureId, GroupId, JsonPatchOp, PresetId, SequenceId,
        UniverseId, Vec3,
    };

    /// **A colour-only fixture has a dimmer the programmer can reach** — S43.
    ///
    /// The desk supplies it, so `attribute_def` answers for it: a value on it
    /// may exist, it rests at nought, and it is on the intensity bank. Without
    /// that answer `Programmer::set_attribute` would skip every fixture in a rig
    /// of PARs and `1 at 50` would do nothing at all.
    #[test]
    fn a_colour_only_fixture_has_an_intensity_the_programmer_can_reach() {
        let mut show = Show::new();
        show.embed_fixture_type(par_type()).unwrap();
        show.embed_fixture_type(dimmer_type()).unwrap();
        show.patch_fixture(fixture(1, "generic.rgbw.par", 1, 1))
            .unwrap();
        show.patch_fixture(fixture(2, "generic.dimmer", 1, 20))
            .unwrap();

        let supplied = show
            .attribute_def(FixtureId::new(1), AttributeType::Dimmer)
            .expect("the desk supplies one for a PAR with no intensity");
        assert_eq!(supplied.default_value, 0, "dark at home");
        assert_eq!(supplied.feature_group, prism_domain::FeatureGroup::Dimmer);
        // The colour it sits over is unchanged: where a colour rests is the
        // **profile's** answer (punch-list B1 set the library's to full) and
        // supplying an intensity does not touch it.
        assert!(
            show.fixture_type("generic.rgbw.par")
                .unwrap()
                .attributes
                .contains(
                    show.attribute_def(FixtureId::new(1), AttributeType::Red)
                        .unwrap()
                ),
            "red is still the profile's own definition"
        );

        // A fixture with a real dimmer keeps the profile's own definition, and
        // that is the one with a channel behind it.
        let real = show
            .attribute_def(FixtureId::new(2), AttributeType::Dimmer)
            .expect("the profile has one");
        assert!(
            show.fixture_type("generic.dimmer")
                .unwrap()
                .attributes
                .contains(real),
            "the profile's own definition, not the supplied one"
        );
    }

    /// Switched off in the patch, the desk supplies nothing and the fixture has
    /// no intensity at all — which is what an operator asks for when the PAR is
    /// on a dimmer pack.
    #[test]
    fn a_fixture_with_the_supplied_intensity_switched_off_has_none() {
        let mut show = Show::new();
        show.embed_fixture_type(par_type()).unwrap();
        let mut patched = fixture(1, "generic.rgbw.par", 1, 1);
        patched.software_dimmer = false;
        show.patch_fixture(patched).unwrap();
        assert!(
            show.attribute_def(FixtureId::new(1), AttributeType::Dimmer)
                .is_none()
        );
    }

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
            // S40's, which a desk reads as often as any of the above: a line
            // that names something twice, or nothing at all, is answered with a
            // sentence rather than with a silence.
            ShowError::FixtureNumberInUse(FixtureId::new(1)),
            ShowError::UnknownLibraryType("generic.par".to_owned()),
            ShowError::UnknownCue {
                sequence: SequenceId::new(1),
                number: "5".to_owned(),
            },
            ShowError::SequenceNumberInUse(SequenceId::new(1)),
            ShowError::NegativeTime(-1.0),
            ShowError::MismatchedObjects {
                from: "cue 2".to_owned(),
                to: "group 6".to_owned(),
            },
            ShowError::SameObject("cue 3".to_owned()),
            ShowError::NotColourable("group"),
            ShowError::NoSelectedSequence,
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

    /// **Embedding a key the show already carries replaces it** — and this is
    /// how a show is brought up to date after the library is corrected.
    ///
    /// A show embeds the profiles it uses (S11) so that it opens the same on a
    /// desk with a different library. The cost is that a copy embedded before a
    /// library fix goes on being used, and S43 found what that costs: punch-list
    /// B1 gave colour channels a home value of full, and an existing rig went on
    /// resting them at nought because nothing replaced its copy. The interface's
    /// half of that fault was skipping the embed when the key was already
    /// present; this is the daemon's half, and it was never asserted either.
    #[test]
    fn embedding_a_profile_again_replaces_the_copy_the_show_was_carrying() {
        // Two copies of one key, differing only in where the colours rest.
        // Written out here rather than taken from the library: what is asserted
        // is the **replacement**, and a test that read the library's values
        // would be asserting two things and reporting one.
        let mut open = par_type();
        for def in &mut open.attributes {
            if def.feature_group == prism_domain::FeatureGroup::Color {
                def.default_value = u16::MAX;
            }
        }
        let mut show = Show::new();
        show.embed_fixture_type(par_type()).unwrap();
        show.patch_fixture(fixture(1, "generic.rgbw.par", 1, 1))
            .unwrap();
        let home = |show: &Show| {
            show.fixture_type("generic.rgbw.par")
                .expect("it is embedded")
                .attributes
                .iter()
                .find(|def| def.attribute == AttributeType::Red)
                .expect("it is red")
                .default_value
        };
        assert_eq!(home(&show), 0, "the show starts on the stale copy");

        // The corrected profile, embedded over it.
        let ops = show.embed_fixture_type(open).unwrap();
        assert_eq!(home(&show), u16::MAX, "the copy was replaced, not kept");
        // A **replace** rather than an add, so a client's mirror follows it.
        assert!(
            matches!(ops.as_slice(), [JsonPatchOp::Replace { path, .. }]
                if path == "/fixtureTypes/generic.rgbw.par"),
            "{ops:?}"
        );
        // And the fixture standing on it is untouched — it is the same profile
        // key, so the patch does not move.
        assert_eq!(show.fixtures().count(), 1);
        assert_eq!(show.fixture(FixtureId::new(1)).unwrap().address, 1);
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
