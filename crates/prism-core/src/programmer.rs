//! The programmer: what the operator has touched but not yet stored.
//!
//! `docs/DMX_MERGE.md` §3. This is the third applier in the crate, and it has
//! the shape of the other two — [`Show::apply`](crate::Show::apply) and
//! [`SessionState::apply`](crate::SessionState::apply): it validates, it
//! applies, it answers with the delta that describes what changed, and a
//! rejection leaves the state byte-identical. All three matches name every one
//! of the 40 commands without a wildcard, so a command added to the protocol is
//! a compile error in three places rather than a silent rejection in any of
//! them.
//!
//! # Sparse is the meaning of the layer, not an optimisation
//!
//! An attribute with **no** programmer value means "the playbacks decide"; an
//! attribute with one is an absolute override of everything below it. A
//! programmer that laid down zeros when an operator selected a fixture would
//! therefore black the stage out — which is why nothing in here ever writes a
//! value that was not asked for, and why `tests/programmer.rs` asserts absence
//! rather than zero.
//!
//! # Where it sits
//!
//! Beside [`Show`] and [`SessionState`](crate::SessionState), held by
//! [`ShowFile`](crate::ShowFile) — and **not** written to the file. A programmer
//! restored from disk would be an absolute override of every playback, applied
//! to a rig the moment the show opened and asked for by nobody. The other half
//! of the argument is the Save LED: setting a value is not an edit to the show
//! (S11 already decided that `SetAttribute` leaves the show clean), so a
//! programmer inside the file would change the file's bytes without the lamp
//! ever lighting.
//!
//! # What needs the show, and why it is passed in rather than held
//!
//! Half of what the programmer decides is the show's knowledge: whether a
//! fixture is patched, whether its profile even has the attribute being set,
//! what that attribute's home value is, and which feature group the profile
//! files it under — which is the profile's answer and not the attribute name's
//! (S6). A `&Show` per call keeps that knowledge in one place instead of
//! copying it here.

use core::fmt;

use prism_domain::{
    AttributeType, ClearStage, Command, Cue, CuePart, CueTrigger, Delta, FeatureGroup, FixtureId,
    GroupId, Preset, PresetId, PresetPool, PresetValue, ProgrammerState, ProgrammerValue,
    ProgrammerValueSource, RgbColor, SelectionMode, Sequence, SequenceId, SequenceStoreMode,
    StoreMode,
};

use crate::command::Applied;
use crate::show::Show;

/// The fade a cue stored from the programmer starts life with.
///
/// A store is a snapshot of a look, not a statement about time: the operator
/// sets the times afterwards in the cue editor, and until they do, a cue does
/// what they just saw. **S28 requirement:** a configurable default cue time is
/// a setting, and it belongs with the cue editor rather than here.
const DEFAULT_FADE_SECONDS: f64 = 0.0;

/// Why a programmer edit was refused.
///
/// Every variant leaves the programmer exactly as it was: an edit builds its
/// successor beside the current state and only swaps it in once nothing can
/// fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgrammerError {
    /// A fixture number that is not patched.
    UnknownFixture(FixtureId),
    /// A preset number that does not exist.
    UnknownPreset(PresetId),
    /// A group number that does not exist (S40).
    UnknownGroup(GroupId),
    /// A sequence number that does not exist.
    UnknownSequence(SequenceId),
    /// An absolute attribute value outside `0..=65535`.
    ValueOutOfRange(i32),
    /// A store with nothing in the programmer to store.
    NothingToStore,
    /// A [`StoreMode::Remove`] against a cue that is not there, or that holds
    /// none of the values the programmer is holding (S39).
    ///
    /// Refused rather than accepted as a no-op, because a store that writes
    /// nothing and removes nothing is one an operator would press twice, and the
    /// second press is the one that reaches for a different button.
    NothingToRemove {
        /// What was named, in the words an operator would read it in — `cue 3
        /// of sequence 1`, or `preset 4`. A string rather than a pair, because
        /// the same refusal covers a cue and a preset and a variant per target
        /// would be two refusals that say the same thing.
        what: String,
    },
    /// A [`SequenceStoreMode::Merge`] into a sequence with no cues in it (S39).
    NoCuesToMergeInto(SequenceId),
    /// An `Update` with no cue loaded — nothing is being edited (S39).
    NothingIsBeingEdited,
    /// A line that names no cue list, on a desk with none selected (S40).
    ///
    /// `Store Cue 5` means `Session::selectedSequence` and
    /// [`ShowFile::apply`](crate::ShowFile::apply) resolves it; this is what a
    /// caller who applied the command straight to a [`Programmer`] gets, because
    /// this type has no session to ask.
    NoSelectedSequence,
    /// A command that is not one of the programmer's.
    NotAProgrammerCommand,
}

impl fmt::Display for ProgrammerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFixture(id) => write!(f, "fixture {id} is not patched"),
            Self::UnknownPreset(id) => write!(f, "there is no preset {id}"),
            Self::UnknownGroup(id) => write!(f, "there is no group {id}"),
            Self::UnknownSequence(id) => write!(f, "there is no sequence {id}"),
            Self::ValueOutOfRange(value) => {
                write!(
                    f,
                    "{value} is not an attribute value: the range is 0..=65535"
                )
            }
            Self::NothingToStore => {
                write!(f, "the programmer is empty, so there is nothing to store")
            }
            Self::NothingToRemove { what } => {
                write!(
                    f,
                    "{what} holds none of these values, so there is nothing to remove"
                )
            }
            Self::NoCuesToMergeInto(id) => {
                write!(f, "sequence {id} has no cues to merge into")
            }
            Self::NoSelectedSequence => {
                write!(
                    f,
                    "no cue list is selected: say which one, or select one first"
                )
            }
            Self::NothingIsBeingEdited => {
                write!(f, "no cue is loaded, so there is nothing to update")
            }
            Self::NotAProgrammerCommand => {
                write!(f, "this command does not belong to the programmer")
            }
        }
    }
}

impl core::error::Error for ProgrammerError {}

/// The operator's live edit: a selection, the values touched on it, and the
/// stage the Clear button has reached.
///
/// The state itself is `prism_domain::ProgrammerState`, because it travels
/// whole — `Delta::ProgrammerChanged` carries the state rather than a patch,
/// which `docs/IPC_PROTOCOL.md` §6 justifies in four words: "it is small and
/// sparse". This type is the rules around it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Programmer {
    state: ProgrammerState,
}

impl Programmer {
    /// An empty programmer: nothing selected, nothing touched, stage 0.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    // -- queries ----------------------------------------------------------

    /// The state itself — what `Delta::ProgrammerChanged` carries.
    #[must_use]
    pub const fn state(&self) -> &ProgrammerState {
        &self.state
    }

    /// The feature groups the programmer holds values for, in encoder-bank
    /// order.
    ///
    /// The group is the one the *profile* files the attribute under, not the
    /// one its name suggests (S6). **S26 requirement:** this is what the
    /// encoder bar marks, so an operator can see that they have touched colour
    /// on a selection they are now looking at through the position bank.
    #[must_use]
    pub fn feature_groups(&self, show: &Show) -> Vec<FeatureGroup> {
        FeatureGroup::ALL
            .into_iter()
            .filter(|group| {
                self.state.values.iter().any(|(&fixture, attributes)| {
                    attributes.keys().any(|&attribute| {
                        show.attribute_def(fixture, attribute)
                            .is_some_and(|def| def.feature_group == *group)
                    })
                })
            })
            .collect()
    }

    /// Touched values the show can no longer resolve, in a stable order.
    ///
    /// A fixture that has been unpatched, or one whose profile was replaced by
    /// a mode without that attribute. S6 drops such values on the way into the
    /// engine and asked this session to surface them: dropped silently, an
    /// operator cannot learn why a value does nothing.
    #[must_use]
    pub fn unresolved(&self, show: &Show) -> Vec<(FixtureId, AttributeType)> {
        self.state
            .values
            .iter()
            .flat_map(|(&fixture, attributes)| {
                attributes
                    .keys()
                    .map(move |&attribute| (fixture, attribute))
            })
            .filter(|&(fixture, attribute)| show.attribute_def(fixture, attribute).is_none())
            .collect()
    }

    /// The cue this programmer stores into a sequence, in the mode the operator
    /// chose.
    ///
    /// **The three modes are S39's, and S28 named them here before they
    /// existed.** Until then this merged unconditionally, and the reason was
    /// written in this paragraph: the programmer is sparse by specification, so
    /// a store carries only what was touched this time, and overwriting would
    /// delete every value in the cue the operator did not happen to touch. That
    /// is still true of [`StoreMode::Merge`], which is still the default — what
    /// changed is that the other two are now *asked for* rather than assumed
    /// away.
    ///
    /// - [`StoreMode::Merge`] — the programmer's values are written in and
    ///   everything else is left alone.
    /// - [`StoreMode::Override`] — the cue's parts become **exactly** the
    ///   programmer's.
    /// - [`StoreMode::Remove`] — the programmer's values are taken **out**, and
    ///   their levels are not used at all.
    ///
    /// An existing cue keeps its name, its times and its trigger under all
    /// three: a store is about the look, and a cue's name is
    /// `Command::SetCueProperty`'s.
    ///
    /// Values the show can no longer resolve are left out rather than refused;
    /// see [`Self::unresolved`]. A `presetRef` naming a preset that has since
    /// been deleted is dropped and the value kept, which is the rule S11
    /// already applied to `Show::remove_preset`.
    ///
    /// # Errors
    ///
    /// [`ProgrammerError::UnknownSequence`] if there is no such sequence,
    /// [`ProgrammerError::NothingToStore`] if nothing would be written, or
    /// [`ProgrammerError::NothingToRemove`] for a [`StoreMode::Remove`] against
    /// a cue that is not there or holds none of these values.
    pub fn cue(
        &self,
        show: &Show,
        sequence_id: SequenceId,
        number: &str,
        mode: StoreMode,
    ) -> Result<Cue, ProgrammerError> {
        let Some(sequence) = show.sequence(sequence_id) else {
            return Err(ProgrammerError::UnknownSequence(sequence_id));
        };
        // The operator typed the number, so `" 2 "` and `"2"` are the same cue:
        // two cues that both read as 2 would sort equally (`Cue::compare_numbers`
        // trims) and a Goto could not say which one it meant.
        let number = number.trim();
        let existing = sequence.cues.iter().find(|cue| cue.number == number);

        if mode == StoreMode::Remove {
            // A Remove writes nothing, so its refusals are the other way round
            // from the other two: there has to be something to take away.
            let Some(existing) = existing else {
                return Err(ProgrammerError::NothingToRemove {
                    what: format!("cue {number} of sequence {sequence_id}"),
                });
            };
            let going = self.stored_keys(show, None);
            let mut cue = existing.clone();
            cue.parts
                .retain(|part| !going.contains(&(part.fixture, part.attribute)));
            if cue.parts.len() == existing.parts.len() {
                return Err(ProgrammerError::NothingToRemove {
                    what: format!("cue {number} of sequence {sequence_id}"),
                });
            }
            return Ok(cue);
        }

        let parts = self.cue_parts(show);
        if parts.is_empty() {
            return Err(ProgrammerError::NothingToStore);
        }
        let mut cue = existing.cloned().unwrap_or_else(|| Cue {
            number: number.to_owned(),
            parts: Vec::new(),
            ..blank_cue()
        });
        if mode == StoreMode::Override {
            // Everything the store does not mention goes. The name and the
            // times stay, because they are not the look.
            cue.parts.clear();
        }
        for part in parts {
            match cue.parts.iter_mut().find(|existing| {
                existing.fixture == part.fixture && existing.attribute == part.attribute
            }) {
                Some(existing) => *existing = part,
                None => cue.parts.push(part),
            }
        }
        cue.parts
            .sort_by_key(|part| (part.fixture.get(), part.attribute));
        Ok(cue)
    }

    /// The group this selection would be stored as — S40's `Store Group 3`.
    ///
    /// A group is a list of **fixtures** and not a look, which is what makes
    /// `Group 3` a selection rather than a set of values; the look over the same
    /// fixtures is a preset. So this reads [`ProgrammerState::selection`] and
    /// nothing else, and an empty selection is refused for the reason an empty
    /// programmer refuses a cue store: a group nothing selects is one an
    /// operator would press twice.
    ///
    /// An existing group **keeps its own name**. Renaming one is
    /// `Command::Label`, and a store that renamed as a side effect would take a
    /// name away that somebody typed.
    ///
    /// # Errors
    ///
    /// [`ProgrammerError::NothingToStore`] for an empty selection.
    pub fn group(
        &self,
        show: &Show,
        group_id: GroupId,
        name: &str,
        mode: prism_domain::OverwriteMode,
    ) -> Result<prism_domain::Group, ProgrammerError> {
        let selection = &self.state().selection;
        if selection.is_empty() {
            return Err(ProgrammerError::NothingToStore);
        }
        let existing = show.group(group_id);
        let fixtures = match (existing, mode) {
            (Some(existing), prism_domain::OverwriteMode::Merge) => {
                let mut out = existing.fixtures.clone();
                for fixture in selection {
                    if !out.contains(fixture) {
                        out.push(*fixture);
                    }
                }
                out
            }
            _ => selection.clone(),
        };
        Ok(prism_domain::Group {
            id: group_id,
            name: existing.map_or_else(
                || {
                    if name.is_empty() {
                        format!("Group {group_id}")
                    } else {
                        name.to_owned()
                    }
                },
                |group| group.name.clone(),
            ),
            fixtures,
        })
    }

    /// The whole cue list this programmer stores into a sequence — S39's
    /// `Command::StoreSequence`.
    ///
    /// **The cue list is handed in rather than looked up** (S40), because
    /// `Store Sequence 4` creates one when the number is free and this has to be
    /// able to build against a list that is not in the show yet. Everything
    /// fallible therefore happens before anything is written, which is what
    /// makes a refused store leave the show without a half-made sequence in it.
    ///
    /// [`Self::cue`] one level up, and the three modes mean at the level of
    /// cues what [`StoreMode`]'s three mean at the level of values:
    ///
    /// - [`SequenceStoreMode::Append`] — a **new cue at the highest number**,
    ///   which is one past the highest whole number the list already has, so an
    ///   operator pressing it repeatedly gets `1`, `2`, `3`.
    /// - [`SequenceStoreMode::Override`] — the sequence *becomes* this look: one
    ///   cue, numbered `1`.
    /// - [`SequenceStoreMode::Merge`] — the look is merged into **every** cue.
    ///
    /// The sequence keeps its name and its loop flag under all three.
    ///
    /// # Errors
    ///
    /// [`ProgrammerError::UnknownSequence`] if there is no such sequence,
    /// [`ProgrammerError::NothingToStore`] if the programmer holds nothing the
    /// show can resolve, or [`ProgrammerError::NoCuesToMergeInto`] for a
    /// [`SequenceStoreMode::Merge`] into a cue list with no cues in it.
    pub fn sequence(
        &self,
        show: &Show,
        base: &Sequence,
        mode: SequenceStoreMode,
    ) -> Result<Sequence, ProgrammerError> {
        let sequence = base;
        let sequence_id = base.id;
        let parts = self.cue_parts(show);
        if parts.is_empty() {
            return Err(ProgrammerError::NothingToStore);
        }
        let mut next = sequence.clone();
        match mode {
            SequenceStoreMode::Append => next.cues.push(Cue {
                number: append_number(&next.cues),
                parts,
                ..blank_cue()
            }),
            SequenceStoreMode::Override => {
                next.cues = vec![Cue {
                    number: "1".to_owned(),
                    parts,
                    ..blank_cue()
                }];
            }
            SequenceStoreMode::Merge => {
                if next.cues.is_empty() {
                    return Err(ProgrammerError::NoCuesToMergeInto(sequence_id));
                }
                for cue in &mut next.cues {
                    for part in parts.iter().cloned() {
                        match cue.parts.iter_mut().find(|existing| {
                            existing.fixture == part.fixture && existing.attribute == part.attribute
                        }) {
                            Some(existing) => *existing = part,
                            None => cue.parts.push(part),
                        }
                    }
                    cue.parts
                        .sort_by_key(|part| (part.fixture.get(), part.attribute));
                }
            }
        }
        Ok(next)
    }

    /// Loads a stored cue into the programmer — S39's `Command::EditCue`.
    ///
    /// **Every part, with its `presetRef` kept.** A load that took the values
    /// and dropped the links would break every preset link in the cue the next
    /// time it was stored, and it would break it *invisibly*: nothing looks
    /// different until somebody edits the preset and watches the cue not follow
    /// it. `prism_domain::preset` has claimed since S1 that a linked part
    /// follows later edits of its preset and `Show::relink` made the claim true
    /// in S28; this is the third place that promise can be broken and it is the
    /// easiest one to break by accident.
    ///
    /// The selection becomes the cue's fixtures, in fixture order — a cue is a
    /// look on a set of fixtures, and loading one without selecting them would
    /// leave the operator with values no encoder reaches. The values arrive as
    /// [`ProgrammerValueSource::Recalled`], which is what that variant has been
    /// for since S1.
    ///
    /// A part the show can no longer resolve is left out, exactly as
    /// [`Self::cue`] leaves it out on the way back — so a cue naming a fixture
    /// somebody has unpatched loads as the part of it that still means
    /// something.
    ///
    /// Returns whether anything changed.
    pub fn load_cue(&mut self, show: &Show, cue: &Cue) -> bool {
        let mut next = ProgrammerState {
            active_feature_group: self.state.active_feature_group,
            ..ProgrammerState::default()
        };
        for part in &cue.parts {
            if show.attribute_def(part.fixture, part.attribute).is_none() {
                continue;
            }
            if !next.selection.contains(&part.fixture) {
                next.selection.push(part.fixture);
            }
            next.set_value(
                part.fixture,
                part.attribute,
                ProgrammerValue {
                    value: part.value,
                    // A link is what it came in with. A part with no link is
                    // `Recalled` and nothing more; a part with one keeps it, so
                    // storing the cue back leaves the preset reaching it.
                    source: ProgrammerValueSource::Recalled,
                    preset_ref: part.preset_ref,
                },
            );
        }
        next.selection.sort_by_key(|fixture| fixture.get());
        self.commit(next)
    }

    /// The preset this programmer stores into a pool.
    ///
    /// **The mirror of [`Self::cue`], with two differences that both come from
    /// the command rather than from the session.**
    ///
    /// It is filtered by `pool`. A colour preset stores the colour values and
    /// leaves the position alone, which is what a pool *is* on a console; and
    /// the bank an attribute is filed under is the profile's answer
    /// (`AttributeDef::feature_group`) rather than the attribute name's, which
    /// is the same rule [`Self::feature_groups`] follows.
    ///
    /// **[`PresetPool::Multi`] is the pool with no filter** — S43. It takes
    /// every value the programmer holds, across the categories, so one number
    /// recalls a whole look. It needed no code of its own: `Multi` answers
    /// `None` from `PresetPool::group`, and `None` is the argument
    /// [`Self::touched`] has taken since S28 for a *cue*, which names no pool
    /// either. A cue and a Multi preset take the same values for the same
    /// reason, and they take them through the same line.
    ///
    /// And a store with nothing to store is accepted when the preset already
    /// exists, because `Command::StorePreset` carries the name and the colour —
    /// so an empty programmer is an ordinary relabel. Onto a preset that does
    /// not exist it is refused: an empty preset applies nothing, and an operator
    /// who made one by accident would have no way to tell it from one that did
    /// not work.
    ///
    /// It takes a [`StoreMode`] for the same reason [`Self::cue`] does, and
    /// each of the three means the same thing over a pool's values as it means
    /// over a cue's parts. It has to: `Query::StorePreview` can be asked about a
    /// preset in any of them, and an answer describing an outcome no command can
    /// produce is precisely what S28 refused to ship.
    ///
    /// A value the show can no longer resolve is left out rather than refused.
    ///
    /// # Errors
    ///
    /// [`ProgrammerError::NothingToStore`] if the preset does not exist and the
    /// programmer holds nothing that belongs in this pool, or
    /// [`ProgrammerError::NothingToRemove`] for a [`StoreMode::Remove`] against
    /// a preset that is not there or holds none of these values.
    pub fn preset(
        &self,
        show: &Show,
        id: PresetId,
        pool: PresetPool,
        name: &str,
        color: Option<RgbColor>,
        mode: StoreMode,
    ) -> Result<Preset, ProgrammerError> {
        let values = self.preset_values(show, pool);
        let existing = show.preset(id);
        // **A colour the command does not carry is one that is kept** (S40).
        // `Preset::color` is what the scribble strips show and it is chosen with
        // a picker; a store that dropped it would throw away something nothing
        // else on the desk can put back. The alternative — a client reading the
        // colour off its mirror and sending it with every store — is the
        // read-modify-write S28 refused for a cue, one pool along.
        let color = color.or_else(|| existing.and_then(|preset| preset.color));

        if mode == StoreMode::Remove {
            // The mirror of the cue's Remove, with the preset's own filter on
            // the way in: a colour Remove takes colour values out and leaves
            // the rest of the pool alone.
            let Some(held) = existing else {
                return Err(ProgrammerError::NothingToRemove {
                    what: format!("preset {id}"),
                });
            };
            let going = self.stored_keys(show, pool.group());
            let mut preset = held.clone();
            preset.name = name.to_owned();
            preset.color = color;
            preset
                .values
                .retain(|value| !going.contains(&(value.fixture, value.attribute)));
            if preset.values.len() == held.values.len() {
                return Err(ProgrammerError::NothingToRemove {
                    what: format!("preset {id}"),
                });
            }
            return Ok(preset);
        }

        if values.is_empty() && existing.is_none() {
            return Err(ProgrammerError::NothingToStore);
        }
        let mut preset = Preset {
            id,
            pool,
            name: name.to_owned(),
            color,
            values: match (mode, existing) {
                // An Override keeps the number, the pool, the name and the
                // colour, and nothing else: the pool ends up holding exactly
                // what the programmer holds of it.
                (StoreMode::Override, _) | (_, None) => Vec::new(),
                (_, Some(preset)) => preset.values.clone(),
            },
        };
        for value in values {
            match preset
                .values
                .iter_mut()
                .find(|held| held.fixture == value.fixture && held.attribute == value.attribute)
            {
                Some(held) => *held = value,
                None => preset.values.push(value),
            }
        }
        preset
            .values
            .sort_by_key(|value| (value.fixture.get(), value.attribute));
        Ok(preset)
    }

    /// The touched values that belong in a pool, in fixture then attribute
    /// order — or all of them, for [`PresetPool::Multi`].
    fn preset_values(&self, show: &Show, pool: PresetPool) -> Vec<PresetValue> {
        self.touched(show, pool.group())
            .map(|(fixture, attribute, value)| PresetValue {
                fixture,
                attribute,
                value: value.value,
            })
            .collect()
    }

    /// **What a store would write**, as the keys it would write them under.
    ///
    /// The answer `ShowFile::preview_store` counts against what is already
    /// stored, and it comes from here rather than from the merged result: the
    /// merge *keeps* what it did not touch, so counting the merged cue would
    /// report every untouched value as one this store had replaced.
    ///
    /// `pool` is `None` for a cue, which takes everything, and `Some` for a
    /// preset, which takes its own pool.
    #[must_use]
    pub fn stored_keys(
        &self,
        show: &Show,
        pool: Option<FeatureGroup>,
    ) -> Vec<(FixtureId, AttributeType)> {
        self.touched(show, pool)
            .map(|(fixture, attribute, _)| (fixture, attribute))
            .collect()
    }

    /// The touched values the show can still resolve, optionally of one pool.
    ///
    /// The one filter behind [`Self::cue_parts`], [`Self::preset_values`] and
    /// [`Self::stored_keys`], so a preview and the store it previews cannot
    /// disagree about which values are even candidates. A value the show can no
    /// longer resolve — an unpatched fixture, or a profile without that
    /// attribute — is not one of them; see [`Self::unresolved`], which is what
    /// tells the operator about it.
    fn touched<'a>(
        &'a self,
        show: &'a Show,
        pool: Option<FeatureGroup>,
    ) -> impl Iterator<Item = (FixtureId, AttributeType, &'a ProgrammerValue)> + 'a {
        self.state
            .values
            .iter()
            .flat_map(|(&fixture, attributes)| {
                attributes
                    .iter()
                    .map(move |(&attribute, value)| (fixture, attribute, value))
            })
            .filter(move |&(fixture, attribute, _)| {
                show.attribute_def(fixture, attribute)
                    .is_some_and(|def| pool.is_none_or(|pool| def.feature_group == pool))
            })
    }

    /// The touched values as cue parts, in fixture then attribute order.
    fn cue_parts(&self, show: &Show) -> Vec<CuePart> {
        self.touched(show, None)
            .map(|(fixture, attribute, value)| CuePart {
                fixture,
                attribute,
                value: value.value,
                // A link to a preset that is gone is not a link. The value it
                // put there stays, exactly as `Show::remove_preset` leaves the
                // cue parts that referenced it.
                preset_ref: value.preset_ref.filter(|&id| show.preset(id).is_some()),
            })
            .collect()
    }

    // -- the applier ------------------------------------------------------

    /// Validates a programmer command and applies it.
    ///
    /// A command that changes nothing produces no delta at all: an encoder
    /// turned with nothing selected is the ordinary case, not the edge case,
    /// and a `ProgrammerChanged` carrying the state it already had is a
    /// broadcast to every client that says nothing.
    ///
    /// `StoreCue` is here for the half of it that is programmer state — the
    /// Clear stage — because the cue itself is a *show* edit and this type has
    /// no show to write to. [`ShowFile::apply`](crate::ShowFile::apply)
    /// composes the two: [`Self::cue`] builds the cue, `Show::store_cue` writes
    /// it, and the show write happens first so that a refusal leaves the
    /// programmer untouched.
    ///
    /// # Errors
    ///
    /// [`ProgrammerError`] if the command cannot be applied. **The programmer
    /// is unchanged after any error**, and `tests/programmer.rs` asserts it on
    /// the serialised bytes rather than on the claim.
    pub fn apply(&mut self, command: &Command, show: &Show) -> Result<Applied, ProgrammerError> {
        let changed = match command {
            Command::SelectFixtures { ids, mode } => self.select_fixtures(ids, *mode, show)?,
            Command::SetAttribute {
                attribute,
                value,
                relative,
            } => self.set_attribute(*attribute, *value, *relative, show)?,
            // S40's `Group 3`. The daemon expands it, which is what stops a
            // client sending a selection a second client's edit of that group
            // has already made wrong — and since S43 it is a **switch** rather
            // than a per-fixture toggle, which is what stops a second group
            // punching a hole in the first. See `select_group`.
            Command::SelectGroup { group_id, mode } => self.select_group(*group_id, *mode, show)?,
            Command::ApplyPreset { preset_id } => self.apply_preset(*preset_id, show)?,
            Command::ClearProgrammer => self.clear(),
            Command::StoreCue {
                sequence_id,
                cue_number,
                mode,
            } => {
                // Validated even though the cue is written elsewhere, so that a
                // direct caller cannot get a bare stage reset out of a store
                // that could not have happened.
                let sequence_id = sequence_id.ok_or(ProgrammerError::NoSelectedSequence)?;
                self.cue(show, sequence_id, cue_number, *mode)?;
                self.touch()
            }
            Command::StorePreset {
                preset_id,
                pool,
                name,
                color,
                mode,
            } => {
                // Validated for the same reason `StoreCue` is: a direct caller
                // must not get a bare stage reset out of a store that could not
                // have happened.
                self.preset(
                    show,
                    *preset_id,
                    pool.ok_or(ProgrammerError::NoSelectedSequence)?,
                    name,
                    *color,
                    *mode,
                )?;
                self.touch()
            }
            Command::StoreSequence {
                sequence_id, mode, ..
            } => {
                // Validated for `StoreCue`'s reason, and **only when the cue
                // list is there**: since S40 this command creates one when the
                // number is free, and `ShowFile::apply` is where that happens.
                if let Some(sequence) = show.sequence(*sequence_id) {
                    self.sequence(show, sequence, *mode)?;
                }
                self.touch()
            }
            Command::StoreGroup {
                group_id,
                name,
                mode,
            } => {
                self.group(show, *group_id, name, *mode)?;
                self.touch()
            }
            // The one command that *fills* the programmer rather than reading
            // it. The show has already said the cue exists — this is where it
            // becomes the operator's live edit.
            Command::EditCue {
                sequence_id,
                cue_number,
            } => {
                // `None` is the selected cue list and `ShowFile::apply` resolved
                // it; a direct caller who did not go through the file has not
                // named one, and the show has no session to ask.
                let sequence_id = sequence_id.ok_or(ProgrammerError::NoSelectedSequence)?;
                let Some(cue) = show.cue(sequence_id, cue_number) else {
                    return Err(ProgrammerError::UnknownSequence(sequence_id));
                };
                let cue = cue.clone();
                self.load_cue(show, &cue)
            }
            // `Update` is a store whose target is the **session's**, and this
            // type has no session. `ShowFile::apply` resolves it and writes the
            // cue; all that is left here is the Clear stage, exactly as it is
            // for the three stores above.
            Command::Update => self.touch(),
            // The other commands, named rather than caught by a wildcard: this
            // match is then exhaustive, so a command added to the protocol is a
            // compile error here as well as in `Show::apply` and
            // `SessionState::apply`.
            Command::SetCueProperty { .. }
            | Command::Delete { .. }
            | Command::Copy { .. }
            | Command::Move { .. }
            | Command::Label { .. }
            | Command::Color { .. }
            | Command::AssignExecutor { .. }
            | Command::ConfigureExecutor { .. }
            | Command::ExecutorGo { .. }
            | Command::ExecutorOff { .. }
            | Command::ExecutorOn { .. }
            | Command::Goto { .. }
            | Command::ExecutorButton { .. }
            | Command::SetExecutorMaster { .. }
            | Command::PatchFixture { .. }
            | Command::UnpatchFixture { .. }
            | Command::RenumberFixture { .. }
            | Command::EmbedFixtureType { .. }
            | Command::Oops
            | Command::Redo
            | Command::SaveShow
            | Command::SaveShowAs { .. }
            | Command::OpenShow { .. }
            | Command::NewShow { .. }
            | Command::ExportShow { .. }
            | Command::ImportShow { .. }
            | Command::SelectView { .. }
            | Command::StoreView { .. }
            | Command::NewView { .. }
            | Command::SetWindowPicker { .. }
            | Command::OpenWindow { .. }
            | Command::CloseWindow { .. }
            | Command::FocusWindow { .. }
            | Command::PlaceWindow { .. }
            | Command::SetExecutorPage { .. }
            | Command::SelectExecutor { .. }
            | Command::SetEncoderBank { .. }
            | Command::SetProgrammerPage { .. }
            | Command::SelectProgrammerParam { .. }
            | Command::SelectSequence { .. }
            | Command::CommandLineInput { .. }
            | Command::AddOutput { .. }
            | Command::ConfigureOutput { .. }
            | Command::RemoveOutput { .. }
            | Command::SetOutputEnabled { .. }
            | Command::SetSurfacePort { .. }
            | Command::ConfigureMachine { .. } => {
                return Err(ProgrammerError::NotAProgrammerCommand);
            }
        };
        Ok(self.answer(changed))
    }

    /// The programmer's half of a store that has **already been carried out**.
    ///
    /// Only the Clear stage: the cue or the preset was built and written by
    /// [`ShowFile::apply`](crate::ShowFile::apply) before this is called.
    ///
    /// **It exists because re-deriving the store here would be a question about
    /// a different show** — the one the store has just written. Until S39 that
    /// distinction cost nothing, because merging twice answers the same both
    /// times; [`StoreMode::Remove`] is where it stops being free. A Remove that
    /// has been applied has taken its values out, so asking a second time
    /// whether there is anything to remove answers *no* — and the command would
    /// be refused **after** it had changed the show, which is the one thing a
    /// refusal may never do. Found by `prismd`'s recording, whose replay and
    /// snapshot disagreed about a cue.
    ///
    /// [`Self::apply`] still validates a store, and that is not a duplicate of
    /// this: it is the answer for a **direct** caller, who has no show write in
    /// front of them and must not get a bare stage reset out of a store that
    /// could not have happened.
    pub fn stored(&mut self) -> Applied {
        let changed = self.touch();
        self.answer(changed)
    }

    /// What an edit that did or did not change something answers with.
    ///
    /// A change that changed nothing produces no delta at all: a
    /// `ProgrammerChanged` carrying the state a client already has is a
    /// broadcast that says nothing.
    fn answer(&self, changed: bool) -> Applied {
        if changed {
            Applied {
                deltas: vec![Delta::ProgrammerChanged {
                    state: self.state.clone(),
                }],
                effects: Vec::new(),
            }
        } else {
            Applied::default()
        }
    }

    // -- edits ------------------------------------------------------------

    /// Changes the selection, fixture by fixture.
    ///
    /// `Set` replaces it, `Add` appends what is not already selected, and
    /// `Toggle` removes what is and appends what is not. The selection is a
    /// list in selection order, and no fixture appears in it twice.
    ///
    /// **Every fixture named here is a *direct* pick** and is recorded as one in
    /// [`ProgrammerState::manual_selection`], which is what makes it survive a
    /// group being switched off later — S43, B27. `Set` drops the group
    /// switches with the selection, because a line that names fixtures is a
    /// statement about what the selection *is*.
    ///
    /// Returns whether anything changed.
    ///
    /// # Errors
    ///
    /// [`ProgrammerError::UnknownFixture`] if any of them is not patched — and
    /// then none of them is selected, because a selection half made is worse
    /// than one refused.
    pub fn select_fixtures(
        &mut self,
        ids: &[FixtureId],
        mode: SelectionMode,
        show: &Show,
    ) -> Result<bool, ProgrammerError> {
        for &id in ids {
            if show.fixture(id).is_none() {
                return Err(ProgrammerError::UnknownFixture(id));
            }
        }
        let mut next = self.interaction();
        if mode == SelectionMode::Set {
            next.clear_selection();
        }
        for &id in ids {
            match next.selection.iter().position(|&selected| selected == id) {
                Some(position) if mode == SelectionMode::Toggle => {
                    next.selection.remove(position);
                    // Taken back out by hand, so it is no longer a direct pick.
                    // The group that also holds it keeps its switch down: the
                    // operator took *this fixture* out, not the group.
                    next.manual_selection.retain(|&held| held != id);
                }
                Some(_) => push_once(&mut next.manual_selection, id),
                None => {
                    next.selection.push(id);
                    push_once(&mut next.manual_selection, id);
                }
            }
        }
        Ok(self.commit(next))
    }

    /// Switches a group on or off — S43, punch-list B27.
    ///
    /// # A group is a switch, not a per-fixture toggle
    ///
    /// It used to be the latter: `Group 3` ran every one of its fixtures through
    /// [`Self::select_fixtures`], so a group whose lamps happened to be selected
    /// already *deselected* them. Two overlapping groups therefore could not be
    /// held together — pressing the second punched a hole in the first — which
    /// is the fault the owner reported.
    ///
    /// So the three modes act on the **group**:
    ///
    /// - `Set` — this group and nothing else.
    /// - `Add` — switch it on. Its fixtures join the selection; nothing leaves.
    /// - `Toggle` — on if it was off; if it was on, switch it off and take back
    ///   only the fixtures **no other group that is still on, and no direct
    ///   pick, is holding**.
    ///
    /// That last clause is why the programmer records provenance at all, and it
    /// is the daemon's answer rather than a client's: which fixtures a group
    /// holds is show state, so a client that worked out the subtraction would be
    /// sending a selection that a second client's edit had already made wrong —
    /// the same argument `Command::SelectGroup` makes for not expanding the
    /// group in the first place.
    ///
    /// A group whose fixtures are all selected already still *switches on*, and
    /// that matters: nothing visible happens, and switching it off afterwards
    /// then takes nothing away, because every one of them is held by something
    /// else. Returns whether anything changed — the switch alone counts.
    ///
    /// # Errors
    ///
    /// [`ProgrammerError::UnknownGroup`] if there is no such group, and
    /// [`ProgrammerError::UnknownFixture`] if it holds one that is not patched.
    pub fn select_group(
        &mut self,
        group_id: GroupId,
        mode: SelectionMode,
        show: &Show,
    ) -> Result<bool, ProgrammerError> {
        let Some(group) = show.group(group_id) else {
            return Err(ProgrammerError::UnknownGroup(group_id));
        };
        for &id in &group.fixtures {
            if show.fixture(id).is_none() {
                return Err(ProgrammerError::UnknownFixture(id));
            }
        }
        let members = group.fixtures.clone();
        let mut next = self.interaction();
        let was_on = next.selected_groups.contains(&group_id);

        if mode == SelectionMode::Set {
            next.clear_selection();
        }
        if mode == SelectionMode::Toggle && was_on {
            next.selected_groups.retain(|&held| held != group_id);
            // What the switches that are still down are holding. Read out of the
            // show rather than remembered, so a group edited since it was
            // switched on releases what it no longer holds.
            let mut held_elsewhere: Vec<FixtureId> = next.manual_selection.clone();
            for &other in &next.selected_groups {
                if let Some(group) = show.group(other) {
                    held_elsewhere.extend(group.fixtures.iter().copied());
                }
            }
            next.selection
                .retain(|id| !members.contains(id) || held_elsewhere.contains(id));
            return Ok(self.commit(next));
        }

        push_once(&mut next.selected_groups, group_id);
        for id in members {
            push_once(&mut next.selection, id);
        }
        Ok(self.commit(next))
    }

    /// Sets an attribute on the current selection.
    ///
    /// Only the selected fixtures whose profile actually defines the attribute
    /// are touched: setting pan on a selection of PARs and moving heads moves
    /// the heads and writes nothing for the PARs, which would otherwise be a
    /// value naming a merge slot that does not exist.
    ///
    /// A **relative** move starts from the value the programmer is holding, or
    /// from the attribute's home value if it is holding none — home being the
    /// bottom of the merge stack and the only base this crate can know.
    /// **S17/S27 note:** basing a relative move on what a *playback* is
    /// currently outputting needs the engine's resolved value, which is what
    /// `ProgrammerValueSource::Recalled` exists for and what a later session
    /// wires up.
    ///
    /// A value set by hand is [`ProgrammerValueSource::Manual`] and carries no
    /// preset link, even where it lands on top of one that came from a preset:
    /// it is no longer the preset's value, so a later edit of that preset must
    /// not move it.
    ///
    /// Returns whether anything changed.
    ///
    /// # Errors
    ///
    /// [`ProgrammerError::ValueOutOfRange`] if an absolute value is not an
    /// attribute value. A relative value may be either sign and any size — an
    /// encoder turns both ways (S1) and a wheel spun hard saturates.
    pub fn set_attribute(
        &mut self,
        attribute: AttributeType,
        value: i32,
        relative: bool,
        show: &Show,
    ) -> Result<bool, ProgrammerError> {
        let absolute = if relative {
            None
        } else {
            Some(u16::try_from(value).map_err(|_| ProgrammerError::ValueOutOfRange(value))?)
        };
        let mut next = self.interaction();
        let mut group = None;
        for &fixture in &self.state.selection {
            let Some(def) = show.attribute_def(fixture, attribute) else {
                continue;
            };
            let resolved = absolute.unwrap_or_else(|| {
                let base = self
                    .state
                    .value(fixture, attribute)
                    .map_or(def.default_value, |held| held.value);
                nudge(base, value)
            });
            next.set_value(
                fixture,
                attribute,
                ProgrammerValue {
                    value: resolved,
                    source: ProgrammerValueSource::Manual,
                    preset_ref: None,
                },
            );
            group.get_or_insert(def.feature_group);
        }
        if let Some(group) = group {
            next.active_feature_group = group;
        }
        Ok(self.commit(next))
    }

    /// Applies a preset to the current selection.
    ///
    /// `ARCHITECTURE_SPEC.md` §6 and `docs/IPC_PROTOCOL.md` §5 both define the
    /// command as "apply a preset to the current selection", so a preset value
    /// for a fixture that is not selected is not applied. A preset holds a
    /// value per fixture, which is what makes that reading the useful one: the
    /// operator selects the heads they mean and recalls the look stored for
    /// them.
    ///
    /// Each value keeps its `presetRef`, which is the whole point of the field
    /// and this session's second exit criterion: a cue stored from here carries
    /// the link, so editing the preset later moves the cue with it.
    ///
    /// Returns whether anything changed.
    ///
    /// # Errors
    ///
    /// [`ProgrammerError::UnknownPreset`] if there is no such preset.
    pub fn apply_preset(
        &mut self,
        preset_id: PresetId,
        show: &Show,
    ) -> Result<bool, ProgrammerError> {
        let Some(preset) = show.preset(preset_id) else {
            return Err(ProgrammerError::UnknownPreset(preset_id));
        };
        let mut next = self.interaction();
        let mut group = None;
        for stored in &preset.values {
            if !self.state.selection.contains(&stored.fixture) {
                continue;
            }
            let Some(def) = show.attribute_def(stored.fixture, stored.attribute) else {
                continue;
            };
            next.set_value(
                stored.fixture,
                stored.attribute,
                ProgrammerValue {
                    value: stored.value,
                    source: ProgrammerValueSource::Preset,
                    preset_ref: Some(preset_id),
                },
            );
            group.get_or_insert(def.feature_group);
        }
        if let Some(group) = group {
            next.active_feature_group = group;
        }
        Ok(self.commit(next))
    }

    /// Clears the first thing there is to clear (`docs/DMX_MERGE.md` §3.1).
    ///
    /// | Stage | What a press takes away |
    /// |---|---|
    /// | `Values` | the programmer values, keeping the selection |
    /// | `Selection` | the selection |
    /// | `All` | the rest — the feature group, and the session's page beside it |
    /// | `Nothing` | nothing: there is nothing to clear |
    ///
    /// # It reads the contents, and it is not a cycle — S43, B2
    ///
    /// The stage used to be a counter on the button, so a press advanced it
    /// whether or not there was anything to clear and the cycle wrapped back to
    /// the start. That left the button standing at a stage the programmer had
    /// moved on from: set a value after clearing everything, and the next press
    /// cleared the *selection* instead of the value. The owner's punch list is
    /// where that turned up.
    ///
    /// Now the stage is [`ProgrammerState::stage`] — derived, so it cannot be
    /// stale — and a press at `Nothing` **changes nothing and reports so**,
    /// which is the second half of what B2 asks for.
    ///
    /// **The page state is the session's half of `All`** and
    /// [`ShowFile::apply`](crate::ShowFile::apply) resets it, because
    /// `programmerPage` and `programmerParamIndex` live in
    /// `ARCHITECTURE_SPEC.md` §4.1 rather than here.
    ///
    /// Returns whether anything changed — `false` when there was nothing to
    /// clear, which is new: it used to be always `true`.
    pub fn clear(&mut self) -> bool {
        let mut next = self.state.clone();
        match self.state.stage() {
            ClearStage::Values => next.values.clear(),
            // The provenance goes with it — see `ProgrammerState::
            // clear_selection`. A `selected_groups` left standing over an empty
            // selection would make the next press of that group a *deselect*.
            ClearStage::Selection => next.clear_selection(),
            ClearStage::All => next = ProgrammerState::default(),
            ClearStage::Nothing => return false,
        }
        self.commit(next)
    }

    /// Replaces the whole state.
    ///
    /// **S14's door.** Every programmer command is undoable
    /// (`Command::is_undoable`), and the inverse of one is the state that was
    /// there before — the programmer is small and sparse, so the journal can
    /// hold whole states rather than describing how to walk one back.
    ///
    /// Returns whether anything changed.
    pub fn restore(&mut self, state: ProgrammerState) -> bool {
        // Not through `interaction`: restoring a state means restoring its
        // Clear stage as well, or an undo would leave the button somewhere the
        // operator never put it.
        self.commit(state)
    }

    // -- internals --------------------------------------------------------

    /// The successor an interaction that is *not* the Clear button starts from.
    ///
    /// It is a plain clone since **S43**. The stage used to be reset here —
    /// `docs/DMX_MERGE.md` §3.1's *the stage resets on any other programmer
    /// interaction*, which existed because the stage was a counter that could
    /// otherwise stand at a number the contents had moved past. A derived stage
    /// cannot: it already reads the contents, so an operator who clears once and
    /// then grabs a fader finds the key showing *values* again with nothing
    /// having reset it. The rule is now a property of the definition rather than
    /// a line every edit has to route through.
    fn interaction(&self) -> ProgrammerState {
        self.state.clone()
    }

    /// An interaction that changes nothing at all.
    ///
    /// Kept as the name for *this command touched the programmer and moved
    /// nothing*, which several arms of [`Self::apply`] answer with. It reports
    /// `false` now, because there is no longer a stage for it to move.
    fn touch(&mut self) -> bool {
        let next = self.interaction();
        self.commit(next)
    }

    /// Swaps in a successor, reporting whether it differs.
    ///
    /// Change detection lives here so no edit has to remember it, exactly as
    /// `SessionState::commit` does for the session. There is no encoding step
    /// to fail before the write: `Delta::ProgrammerChanged` carries the state
    /// itself, and nothing in a `ProgrammerState` is a float.
    fn commit(&mut self, mut next: ProgrammerState) -> bool {
        // **The one place the Clear stage is written** — S43, B2. It is derived
        // from the contents rather than counted on the button, so every path
        // that changes the programmer brings it along and none of them has to
        // remember to. See `prism_domain::ClearStage`.
        next.restage();
        if next == self.state {
            return false;
        }
        self.state = next;
        true
    }
}

/// A cue with nothing set: the times, the trigger and the name a store gives a
/// cue it is creating.
///
/// One function rather than three literals, because [`Programmer::cue`] and
/// [`Programmer::sequence`] both create cues and a default that differed between
/// them would be two answers to *what does a fresh cue do*.
fn blank_cue() -> Cue {
    Cue {
        number: String::new(),
        name: String::new(),
        fade_in: DEFAULT_FADE_SECONDS,
        fade_out: DEFAULT_FADE_SECONDS,
        delay: 0.0,
        trigger: CueTrigger::Go,
        trigger_time: None,
        parts: Vec::new(),
    }
}

/// The number [`SequenceStoreMode::Append`] gives the cue it adds.
///
/// **One past the highest whole number in the list**, so `1`, `1.5`, `2` gains a
/// `3` and an operator pressing Append over and over gets `1`, `2`, `3`. Not one
/// past the highest number *of any kind*, which would answer `2.5` there and
/// give a show a running order nobody would write by hand; and not the count of
/// cues, which would collide the moment a cue was deleted.
///
/// A cue number that is not a number at all sorts after every number that is
/// (`Cue::compare_numbers`) and is ignored here for the same reason: it is a
/// label, and the next label is not a thing arithmetic can find.
fn append_number(cues: &[Cue]) -> String {
    let highest = cues
        .iter()
        .filter_map(|cue| cue.number.trim().parse::<f64>().ok())
        .fold(0.0_f64, f64::max);
    format!("{}", highest.floor() as i64 + 1)
}

/// Appends `value` unless the list already holds it.
///
/// The three selection lists are ordered sets: the order is the order things
/// were pressed in, which is what a `thru` reads and what an operator sees, and
/// a duplicate in any of them would make a group's switch need pressing twice.
fn push_once<T: PartialEq>(list: &mut Vec<T>, value: T) {
    if !list.contains(&value) {
        list.push(value);
    }
}

/// A relative move, saturating at both ends of the attribute range.
///
/// The delta is an `i32` because an encoder turns both ways and a client may
/// send an arbitrarily large one; the result is an attribute value.
fn nudge(base: u16, delta: i32) -> u16 {
    if delta >= 0 {
        base.saturating_add(u16::try_from(delta).unwrap_or(u16::MAX))
    } else {
        base.saturating_sub(u16::try_from(delta.unsigned_abs()).unwrap_or(u16::MAX))
    }
}

#[cfg(test)]
mod tests {
    use super::{Programmer, ProgrammerError, nudge};
    use crate::testkit::{cue, dimmer_type, executor, fixture, par_type, preset, sequence};
    use crate::{Show, ShowFile};
    use prism_domain::{
        AttributeType, ClearStage, Command, Delta, FeatureGroup, FixtureId, GroupId, Preset,
        PresetId, PresetPool, PresetValue, ProgrammerState, ProgrammerValue, ProgrammerValueSource,
        SelectionMode, SequenceId, SequenceStoreMode, StoreMode,
    };

    /// Three PARs and a dimmer, one preset, one sequence.
    fn show() -> Show {
        let mut show = Show::new();
        show.embed_fixture_type(par_type()).unwrap();
        show.embed_fixture_type(dimmer_type()).unwrap();
        for id in 1..=3 {
            show.patch_fixture(fixture(id, "generic.rgbw.par", 1, (id as u16 - 1) * 4 + 1))
                .unwrap();
        }
        show.patch_fixture(fixture(4, "generic.dimmer", 2, 1))
            .unwrap();
        show.store_preset(preset(4, 1, AttributeType::Red, 65535))
            .unwrap();
        show.store_sequence(sequence(1, vec![cue("1", 1, AttributeType::Red, 65535)]))
            .unwrap();
        show.store_executor(executor(0, Some(1))).unwrap();
        show.mark_saved();
        show
    }

    /// A programmer holding red on fixtures 1 and 2.
    fn programmer(show: &Show) -> Programmer {
        let mut programmer = Programmer::new();
        programmer
            .select_fixtures(
                &[FixtureId::new(1), FixtureId::new(2)],
                SelectionMode::Set,
                show,
            )
            .unwrap();
        programmer
            .set_attribute(AttributeType::Red, 65535, false, show)
            .unwrap();
        programmer
    }

    #[test]
    fn an_edit_that_changes_nothing_says_nothing() {
        let show = show();
        let mut programmer = programmer(&show);
        // The same selection again, and the same value again.
        assert!(
            !programmer
                .select_fixtures(
                    &[FixtureId::new(1), FixtureId::new(2)],
                    SelectionMode::Set,
                    &show
                )
                .unwrap()
        );
        assert!(
            !programmer
                .set_attribute(AttributeType::Red, 65535, false, &show)
                .unwrap()
        );
        // And nothing at all through the applier.
        assert_eq!(
            programmer
                .apply(
                    &Command::SetAttribute {
                        attribute: AttributeType::Red,
                        value: 65535,
                        relative: false,
                    },
                    &show
                )
                .unwrap()
                .deltas,
            vec![]
        );
    }

    #[test]
    fn an_applied_command_carries_the_whole_state() {
        // `docs/IPC_PROTOCOL.md` §6: sent whole, because it is small and
        // sparse. Not a JSON Patch, so there is no document to point into.
        let show = show();
        let mut programmer = Programmer::new();
        let applied = programmer
            .apply(
                &Command::SelectFixtures {
                    ids: vec![FixtureId::new(1)],
                    mode: SelectionMode::Set,
                },
                &show,
            )
            .unwrap();
        assert_eq!(
            applied.deltas,
            vec![Delta::ProgrammerChanged {
                state: programmer.state().clone()
            }]
        );
        assert!(applied.effects.is_empty());
    }

    /// A show with two overlapping groups: 1 holds fixtures 1 and 2, 2 holds
    /// 2 and 3. The overlap is the whole point — fixture 2 is what the old
    /// per-fixture toggle got wrong.
    fn show_with_overlapping_groups() -> Show {
        let mut show = show();
        show.store_group(prism_domain::Group {
            id: GroupId::new(1),
            name: "Front".to_owned(),
            fixtures: vec![FixtureId::new(1), FixtureId::new(2)],
        })
        .unwrap();
        show.store_group(prism_domain::Group {
            id: GroupId::new(2),
            name: "Back".to_owned(),
            fixtures: vec![FixtureId::new(2), FixtureId::new(3)],
        })
        .unwrap();
        show
    }

    /// **Two overlapping groups can be held at once** — S43, punch-list B27.
    ///
    /// This is the fault the owner reported, written as a test: under the old
    /// per-fixture toggle, switching on a second group that shared a fixture
    /// with the first *deselected* the shared one. A group is a switch now, so
    /// the second press only ever adds.
    #[test]
    fn a_second_group_does_not_punch_a_hole_in_the_first() {
        let show = show_with_overlapping_groups();
        let mut programmer = Programmer::new();
        programmer
            .select_group(GroupId::new(1), SelectionMode::Toggle, &show)
            .unwrap();
        programmer
            .select_group(GroupId::new(2), SelectionMode::Toggle, &show)
            .unwrap();
        assert_eq!(
            programmer.state().selection,
            vec![FixtureId::new(1), FixtureId::new(2), FixtureId::new(3)],
            "the fixture the two groups share was toggled back out"
        );
        assert_eq!(
            programmer.state().selected_groups,
            vec![GroupId::new(1), GroupId::new(2)]
        );
    }

    /// **Switching a group off leaves what something else is holding.**
    ///
    /// The other half of B27, and the reason the programmer records provenance
    /// at all: fixture 2 is in both groups, so turning group 1 off must leave it
    /// selected — group 2 is still on.
    #[test]
    fn switching_a_group_off_keeps_what_another_group_still_holds() {
        let show = show_with_overlapping_groups();
        let mut programmer = Programmer::new();
        for id in [1, 2] {
            programmer
                .select_group(GroupId::new(id), SelectionMode::Toggle, &show)
                .unwrap();
        }
        programmer
            .select_group(GroupId::new(1), SelectionMode::Toggle, &show)
            .unwrap();
        assert_eq!(
            programmer.state().selection,
            vec![FixtureId::new(2), FixtureId::new(3)],
            "fixture 1 was group 1's alone; fixture 2 is still group 2's"
        );
        assert_eq!(programmer.state().selected_groups, vec![GroupId::new(2)]);
    }

    /// A fixture picked by hand outlives the group that also held it.
    ///
    /// The provenance is two lists and not one: *held by a group* and *picked
    /// directly* are different claims, and a fixture that is both must survive
    /// either one going away.
    #[test]
    fn switching_a_group_off_keeps_what_was_picked_by_hand() {
        let show = show_with_overlapping_groups();
        let mut programmer = Programmer::new();
        programmer
            .select_group(GroupId::new(1), SelectionMode::Toggle, &show)
            .unwrap();
        // Already selected by the group; picking it again says *and this one*,
        // which is what makes it survive below.
        programmer
            .select_fixtures(&[FixtureId::new(2)], SelectionMode::Add, &show)
            .unwrap();
        programmer
            .select_group(GroupId::new(1), SelectionMode::Toggle, &show)
            .unwrap();
        assert_eq!(programmer.state().selection, vec![FixtureId::new(2)]);
        assert!(programmer.state().selected_groups.is_empty());
    }

    /// A group edited while its switch is down releases what it no longer holds.
    ///
    /// The subtraction reads the show rather than a remembered member list —
    /// which is the same reason `Command::SelectGroup` names the group instead
    /// of expanding it in the client.
    #[test]
    fn switching_a_group_off_reads_the_group_as_it_is_now() {
        let mut show = show_with_overlapping_groups();
        let mut programmer = Programmer::new();
        for id in [1, 2] {
            programmer
                .select_group(GroupId::new(id), SelectionMode::Toggle, &show)
                .unwrap();
        }
        // Group 2 loses the fixture it shared, so group 1 is now the only thing
        // holding fixture 2 and switching it off must take it.
        show.store_group(prism_domain::Group {
            id: GroupId::new(2),
            name: "Back".to_owned(),
            fixtures: vec![FixtureId::new(3)],
        })
        .unwrap();
        programmer
            .select_group(GroupId::new(1), SelectionMode::Toggle, &show)
            .unwrap();
        assert_eq!(programmer.state().selection, vec![FixtureId::new(3)]);
    }

    /// A typed `Group 3` still means *the selection is this group*.
    ///
    /// `Set` is what a line naming a thing means (`patch/sheet.tsx` writes the
    /// same rule down for a fixture), and it drops both halves of the
    /// provenance with the selection — or the group would come back on the next
    /// press of a switch nobody had touched.
    #[test]
    fn a_group_named_without_a_plus_replaces_the_selection() {
        let show = show_with_overlapping_groups();
        let mut programmer = Programmer::new();
        programmer
            .select_fixtures(&[FixtureId::new(4)], SelectionMode::Set, &show)
            .unwrap();
        programmer
            .select_group(GroupId::new(2), SelectionMode::Set, &show)
            .unwrap();
        assert_eq!(
            programmer.state().selection,
            vec![FixtureId::new(2), FixtureId::new(3)]
        );
        assert_eq!(programmer.state().selected_groups, vec![GroupId::new(2)]);
        assert!(programmer.state().manual_selection.is_empty());
    }

    /// Clearing the selection clears the switches with it.
    ///
    /// Otherwise the first press of a group after a Clear would be a *deselect*
    /// of fixtures that were never selected — the switch would be down with
    /// nothing under it.
    #[test]
    fn clearing_the_selection_lifts_every_group_switch() {
        let show = show_with_overlapping_groups();
        let mut programmer = Programmer::new();
        programmer
            .select_group(GroupId::new(1), SelectionMode::Toggle, &show)
            .unwrap();
        // Stage 1 is the values; there are none, so this press takes the
        // selection.
        assert_eq!(programmer.state().stage(), ClearStage::Selection);
        programmer.clear();
        assert!(programmer.state().selection.is_empty());
        assert!(programmer.state().selected_groups.is_empty());
        assert!(programmer.state().manual_selection.is_empty());
    }

    /// **Multi takes every value, which is what a cue takes** — S43.
    ///
    /// The pool with no filter, and the test asserts it against a *filtered*
    /// pool over the same programmer so that the difference is what is being
    /// measured rather than the count.
    #[test]
    fn a_multi_preset_takes_every_value_and_a_colour_one_takes_the_colour() {
        let show = show();
        let mut programmer = Programmer::new();
        // A PAR and a dimmer together: the dimmer has no colour, so the two
        // settings land on different banks.
        //
        // **The PAR has an intensity too, and the desk supplies it** — S43. Its
        // profile has none, so `Show::attribute_def` answers for it and the
        // second line below lands on *both* fixtures. That is what makes this a
        // sharper test of a Multi preset than it was: three values over two
        // banks and two fixtures, where a Colour preset still takes exactly the
        // one that is a colour.
        programmer
            .select_fixtures(
                &[FixtureId::new(1), FixtureId::new(4)],
                SelectionMode::Set,
                &show,
            )
            .unwrap();
        programmer
            .set_attribute(AttributeType::Red, 65535, false, &show)
            .unwrap();
        programmer
            .set_attribute(AttributeType::Dimmer, 30000, false, &show)
            .unwrap();

        let colour = programmer
            .preset(
                &show,
                PresetId::new(9),
                PresetPool::Color,
                "Red",
                None,
                StoreMode::Merge,
            )
            .unwrap();
        assert_eq!(colour.values.len(), 1);
        assert_eq!(colour.values[0].attribute, AttributeType::Red);

        let multi = programmer
            .preset(
                &show,
                PresetId::new(10),
                PresetPool::Multi,
                "The look",
                None,
                StoreMode::Merge,
            )
            .unwrap();
        assert_eq!(multi.pool, PresetPool::Multi);
        assert_eq!(
            multi.values.len(),
            3,
            "a Multi preset takes both banks: the PAR's red and both intensities"
        );
    }

    #[test]
    fn a_selection_never_contains_a_fixture_twice() {
        let show = show();
        let mut programmer = Programmer::new();
        let ids = [FixtureId::new(1), FixtureId::new(1), FixtureId::new(2)];
        programmer
            .select_fixtures(&ids, SelectionMode::Set, &show)
            .unwrap();
        assert_eq!(
            programmer.state().selection,
            vec![FixtureId::new(1), FixtureId::new(2)]
        );
        programmer
            .select_fixtures(&ids, SelectionMode::Add, &show)
            .unwrap();
        assert_eq!(programmer.state().selection.len(), 2);
        // Toggling the same fixture twice in one command leaves it as it was.
        programmer
            .select_fixtures(
                &[FixtureId::new(1), FixtureId::new(1)],
                SelectionMode::Toggle,
                &show,
            )
            .unwrap();
        assert_eq!(programmer.state().selection.len(), 2);
    }

    #[test]
    fn the_direct_api_validates_what_the_show_applier_would_have_caught() {
        // Through `ShowFile::apply` the show refuses these first, so these two
        // guards belong to the callers S14, S17 and S27 will be — a programmer
        // edit that reached a fixture the patch does not have would put an
        // operator's value on nothing at all.
        let show = show();
        let mut programmer = Programmer::new();
        assert_eq!(
            programmer.select_fixtures(&[FixtureId::new(9)], SelectionMode::Set, &show),
            Err(ProgrammerError::UnknownFixture(FixtureId::new(9)))
        );
        assert_eq!(
            programmer.apply_preset(PresetId::new(9), &show),
            Err(ProgrammerError::UnknownPreset(PresetId::new(9)))
        );
        assert_eq!(programmer, Programmer::new());
    }

    #[test]
    fn a_preset_value_for_an_attribute_the_fixture_lacks_is_skipped() {
        // Presets outlive the rigs they were recorded on: the same "Deep blue"
        // applied to a selection that now includes a plain dimmer must move the
        // lights that have the colour and leave the rest alone, rather than
        // writing a value nothing can carry.
        let mut show = show();
        show.store_preset(Preset {
            id: PresetId::new(5),
            pool: PresetPool::Color,
            name: "Deep blue".to_owned(),
            color: None,
            values: vec![
                PresetValue {
                    fixture: FixtureId::new(1),
                    attribute: AttributeType::Blue,
                    value: 65535,
                },
                PresetValue {
                    fixture: FixtureId::new(4),
                    attribute: AttributeType::Blue,
                    value: 65535,
                },
            ],
        })
        .unwrap();

        let mut programmer = Programmer::new();
        programmer
            .select_fixtures(
                &[FixtureId::new(1), FixtureId::new(4)],
                SelectionMode::Set,
                &show,
            )
            .unwrap();
        assert!(programmer.apply_preset(PresetId::new(5), &show).unwrap());
        assert!(
            programmer
                .state()
                .value(FixtureId::new(1), AttributeType::Blue)
                .is_some()
        );
        assert!(
            programmer
                .state()
                .value(FixtureId::new(4), AttributeType::Blue)
                .is_none(),
            "the dimmer has no blue to set"
        );
    }

    #[test]
    fn the_errors_read_as_sentences() {
        assert_eq!(
            ProgrammerError::UnknownFixture(FixtureId::new(9)).to_string(),
            "fixture 9 is not patched"
        );
        assert_eq!(
            ProgrammerError::UnknownPreset(PresetId::new(9)).to_string(),
            "there is no preset 9"
        );
        assert_eq!(
            ProgrammerError::UnknownSequence(SequenceId::new(9)).to_string(),
            "there is no sequence 9"
        );
        assert_eq!(
            ProgrammerError::ValueOutOfRange(-1).to_string(),
            "-1 is not an attribute value: the range is 0..=65535"
        );
        assert_eq!(
            ProgrammerError::NothingToStore.to_string(),
            "the programmer is empty, so there is nothing to store"
        );
        assert_eq!(
            ProgrammerError::NotAProgrammerCommand.to_string(),
            "this command does not belong to the programmer"
        );
    }

    #[test]
    fn a_nudge_saturates_at_both_ends() {
        assert_eq!(nudge(100, 50), 150);
        assert_eq!(nudge(100, -50), 50);
        assert_eq!(nudge(100, -100), 0);
        assert_eq!(nudge(100, -101), 0);
        assert_eq!(nudge(65535, 1), 65535);
        // A delta wider than the range it moves in.
        assert_eq!(nudge(100, i32::MAX), 65535);
        assert_eq!(nudge(100, i32::MIN), 0);
    }

    /// **S43 changed what this asserts, and the reason is B2.** It used to say
    /// that an undone Clear puts the *button's counter* back, because the stage
    /// was remembered state that an Oops had to restore like any other. The
    /// stage is derived now, so there is nothing to put back and nothing that
    /// can be put back wrongly: restoring the look restores the stage, because
    /// the stage is a reading of the look. That is a stronger claim than the old
    /// one and it is what this now asserts.
    #[test]
    fn restoring_puts_back_the_look_and_the_stage_follows_it() {
        let show = show();
        let mut programmer = programmer(&show);
        // Something to clear: the fixture programmer() touched.
        assert_eq!(programmer.state().clear_stage, ClearStage::Values);
        programmer.clear();
        let stored = programmer.state().clone();
        // The values are gone and the selection is not, so the key now offers
        // the selection — read off the contents rather than counted.
        assert_eq!(stored.clear_stage, ClearStage::Selection);
        assert_eq!(stored.clear_stage, stored.stage());

        programmer
            .select_fixtures(&[FixtureId::new(3)], SelectionMode::Add, &show)
            .unwrap();
        // Still nothing but a selection, so still the same offer. The old
        // counter reset to zero here and told the operator there was nothing
        // to clear when there was.
        assert_eq!(programmer.state().clear_stage, ClearStage::Selection);

        assert!(programmer.restore(stored.clone()));
        assert_eq!(programmer.state(), &stored);
        assert!(!programmer.restore(stored));

        // S15 clones a `ShowFile` to write it while the desk goes on being
        // operated, which clones this and its error type with it.
        assert_eq!(programmer.clone(), programmer);
        assert_eq!(
            ProgrammerError::NothingToStore.clone(),
            ProgrammerError::NothingToStore
        );
    }

    #[test]
    fn a_value_whose_preset_has_been_deleted_keeps_the_value_and_loses_the_link() {
        let mut show = show();
        let mut programmer = Programmer::new();
        programmer
            .select_fixtures(&[FixtureId::new(1)], SelectionMode::Set, &show)
            .unwrap();
        programmer.apply_preset(PresetId::new(4), &show).unwrap();
        show.remove_preset(PresetId::new(4)).unwrap();

        let cue = programmer
            .cue(&show, SequenceId::new(1), "2", StoreMode::Merge)
            .unwrap();
        assert_eq!(cue.parts.len(), 1);
        assert_eq!(cue.parts[0].value, 65535);
        assert_eq!(cue.parts[0].preset_ref, None);
        // The programmer itself still remembers where the value came from; it
        // is the *cue* that must not carry a link to nothing.
        assert_eq!(
            programmer
                .state()
                .value(FixtureId::new(1), AttributeType::Red)
                .map(|value| value.source),
            Some(ProgrammerValueSource::Preset)
        );
    }

    #[test]
    fn a_new_cue_starts_with_no_time_and_no_name() {
        let show = show();
        let programmer = programmer(&show);
        let cue = programmer
            .cue(&show, SequenceId::new(1), " 2 ", StoreMode::Merge)
            .unwrap();
        assert_eq!(cue.number, "2", "the typed number is trimmed");
        assert_eq!(cue.name, "");
        assert!(cue.fade_in.abs() < f64::EPSILON);
        assert!(cue.fade_out.abs() < f64::EPSILON);
        assert_eq!(cue.parts.len(), 2);
        // In fixture then attribute order, whatever order they were touched in.
        assert_eq!(cue.parts[0].fixture, FixtureId::new(1));
        assert_eq!(cue.parts[1].fixture, FixtureId::new(2));
    }

    #[test]
    fn storing_into_a_sequence_that_is_not_there_is_refused() {
        let show = show();
        let programmer = programmer(&show);
        assert_eq!(
            programmer.cue(&show, SequenceId::new(9), "1", StoreMode::Merge),
            Err(ProgrammerError::UnknownSequence(SequenceId::new(9)))
        );
        assert_eq!(
            Programmer::new().cue(&show, SequenceId::new(1), "1", StoreMode::Merge),
            Err(ProgrammerError::NothingToStore)
        );
    }

    #[test]
    fn a_value_the_show_can_no_longer_resolve_is_reported_and_left_out() {
        let mut show = show();
        let programmer = programmer(&show);
        show.unpatch_fixture(FixtureId::new(2)).unwrap();

        assert_eq!(
            programmer.unresolved(&show),
            vec![(FixtureId::new(2), AttributeType::Red)]
        );
        let cue = programmer
            .cue(&show, SequenceId::new(1), "2", StoreMode::Merge)
            .unwrap();
        assert_eq!(cue.parts.len(), 1);
        assert_eq!(cue.parts[0].fixture, FixtureId::new(1));
        // It is not counted as a feature group either: the show no longer knows
        // which group that attribute is filed under.
        assert_eq!(programmer.feature_groups(&show), vec![FeatureGroup::Color]);
    }

    #[test]
    fn the_programmer_state_a_daemon_restores_still_prunes_empty_fixtures() {
        // The S1 invariant: a fixture with nothing touched has no representation
        // on the wire, so it must not exist in the map either.
        let show = show();
        let mut programmer = Programmer::new();
        let mut state = ProgrammerState::default();
        state.set_value(
            FixtureId::new(1),
            AttributeType::Red,
            ProgrammerValue {
                value: 1,
                source: ProgrammerValueSource::Manual,
                preset_ref: None,
            },
        );
        programmer.restore(state);
        assert_eq!(programmer.state().values.len(), 1);

        // The Clear that empties the values empties the map with them.
        programmer.clear();
        assert!(programmer.state().values.is_empty());
        assert_eq!(programmer.feature_groups(&show), Vec::new());
    }

    /// **The store commands are validated here as well, for a direct caller.**
    ///
    /// `ShowFile::apply` does not come through this path — it builds the cue
    /// itself and then calls [`Programmer::stored`], because re-deriving a store
    /// against the show it has just written asks a different question (see that
    /// method). What is left here is the answer for somebody calling the
    /// programmer on its own, and it has to be the same answer: a store that
    /// could not have happened may not leave a bare Clear-stage reset behind.
    #[test]
    fn a_store_a_direct_caller_could_not_have_made_is_refused_here_too() {
        let show = show();
        let mut programmer = programmer(&show);
        // The Clear stage is where a refused store could show up, so it is put
        // somewhere a reset would be visible.
        programmer.clear();
        let before = programmer.state().clone();

        for (command, expected) in [
            (
                Command::StoreCue {
                    sequence_id: Some(SequenceId::new(404)),
                    cue_number: "1".to_owned(),
                    mode: StoreMode::Merge,
                },
                ProgrammerError::UnknownSequence(SequenceId::new(404)),
            ),
            (
                Command::StoreCue {
                    sequence_id: Some(SequenceId::new(1)),
                    cue_number: "9".to_owned(),
                    mode: StoreMode::Remove,
                },
                ProgrammerError::NothingToRemove {
                    what: "cue 9 of sequence 1".to_owned(),
                },
            ),
            (
                Command::EditCue {
                    sequence_id: Some(SequenceId::new(1)),
                    cue_number: "404".to_owned(),
                },
                ProgrammerError::UnknownSequence(SequenceId::new(1)),
            ),
        ] {
            assert_eq!(
                programmer.apply(&command, &show),
                Err(expected),
                "{command:?}"
            );
            assert_eq!(programmer.state(), &before, "{command:?} moved the state");
        }

        // A store with an empty pool onto a preset that does not exist.
        let mut empty = Programmer::new();
        assert_eq!(
            empty.apply(
                &Command::StorePreset {
                    preset_id: PresetId::new(9),
                    pool: Some(PresetPool::Color),
                    name: String::new(),
                    color: None,
                    mode: StoreMode::Merge,
                },
                &show,
            ),
            Err(ProgrammerError::NothingToStore)
        );
    }

    /// A store that *can* happen touches the Clear stage and **nothing else** —
    /// and from an idle stage that is no change at all, so it is no delta
    /// either.
    ///
    /// S13 proved that a store can never meet a non-zero Clear stage: pressing
    /// Clear once takes the values with it, so a programmer with something to
    /// store is a programmer whose key offers its values. **S43** made the stage
    /// derived, so a store cannot move it at all — a store changes neither the
    /// values nor the selection — and the last three lines say so.
    #[test]
    fn a_store_leaves_the_look_in_the_programmer_and_says_nothing_it_need_not() {
        let show = show();
        let mut programmer = programmer(&show);
        let before = programmer.state().clone();
        assert_eq!(before.clear_stage, ClearStage::Values);

        for command in [
            Command::StoreCue {
                sequence_id: Some(SequenceId::new(1)),
                cue_number: "1".to_owned(),
                mode: StoreMode::Merge,
            },
            Command::StorePreset {
                preset_id: PresetId::new(4),
                pool: Some(PresetPool::Color),
                name: "Deep red".to_owned(),
                color: None,
                mode: StoreMode::Merge,
            },
            Command::StoreSequence {
                sequence_id: SequenceId::new(1),
                name: String::new(),
                mode: SequenceStoreMode::Merge,
            },
            // And `Update`, which this type cannot validate at all: its target
            // is the session's, and `ShowFile::apply` is what resolves it.
            Command::Update,
        ] {
            let applied = programmer
                .apply(&command, &show)
                .unwrap_or_else(|error| panic!("{command:?}: {error}"));
            assert!(
                applied.deltas.is_empty(),
                "{command:?} broadcast a state that had not moved"
            );
            assert_eq!(programmer.state(), &before, "{command:?} moved the look");
        }

        // `Programmer::stored` is the same answer with no re-derivation in
        // front of it — which is the whole difference, and the reason
        // `ShowFile::apply` uses it. **It never reports a change now**: the only
        // thing it used to move was the Clear stage, and the stage is a reading
        // of contents a store does not touch.
        assert!(programmer.stored().deltas.is_empty());
        let mut cleared = programmer.clone();
        cleared.clear();
        assert_eq!(cleared.state().clear_stage, ClearStage::Selection);
        assert!(cleared.stored().deltas.is_empty());
        assert_eq!(cleared.state().clear_stage, ClearStage::Selection);
    }

    /// Every refusal says what was wrong, in words an operator can read.
    #[test]
    fn the_new_refusals_read_as_themselves() {
        assert_eq!(
            ProgrammerError::NothingToRemove {
                what: "cue 3 of sequence 1".to_owned(),
            }
            .to_string(),
            "cue 3 of sequence 1 holds none of these values, so there is nothing to remove"
        );
        assert_eq!(
            ProgrammerError::NoCuesToMergeInto(SequenceId::new(9)).to_string(),
            "sequence 9 has no cues to merge into"
        );
        assert_eq!(
            ProgrammerError::NothingIsBeingEdited.to_string(),
            "no cue is loaded, so there is nothing to update"
        );
    }

    /// **A cue part the show can no longer resolve is left out of the load**,
    /// exactly as it is left out of the store — so a cue naming a fixture
    /// somebody has unpatched loads as the part of it that still means
    /// something.
    #[test]
    fn loading_a_cue_leaves_out_what_the_show_can_no_longer_resolve() {
        let mut show = show();
        show.store_sequence(sequence(
            2,
            vec![prism_domain::Cue {
                number: "1".to_owned(),
                name: String::new(),
                fade_in: 0.0,
                fade_out: 0.0,
                delay: 0.0,
                trigger: prism_domain::CueTrigger::Go,
                trigger_time: None,
                parts: vec![
                    prism_domain::CuePart {
                        fixture: FixtureId::new(1),
                        attribute: AttributeType::Red,
                        value: 11,
                        preset_ref: None,
                    },
                    prism_domain::CuePart {
                        fixture: FixtureId::new(2),
                        attribute: AttributeType::Red,
                        value: 22,
                        preset_ref: None,
                    },
                ],
            }],
        ))
        .unwrap();
        show.unpatch_fixture(FixtureId::new(2)).unwrap();

        let mut programmer = Programmer::new();
        assert!(
            programmer
                .apply(
                    &Command::EditCue {
                        sequence_id: Some(SequenceId::new(2)),
                        cue_number: "1".to_owned(),
                    },
                    &show,
                )
                .unwrap()
                .deltas
                .len()
                == 1
        );
        assert_eq!(programmer.state().selection, vec![FixtureId::new(1)]);
        assert_eq!(programmer.state().values.len(), 1);
        assert!(
            programmer
                .state()
                .value(FixtureId::new(2), AttributeType::Red)
                .is_none()
        );

        // And loading the same cue twice changes nothing the second time, which
        // is what stops a repeated Edit broadcasting a delta that says nothing.
        assert!(
            programmer
                .apply(
                    &Command::EditCue {
                        sequence_id: Some(SequenceId::new(2)),
                        cue_number: "1".to_owned(),
                    },
                    &show,
                )
                .unwrap()
                .deltas
                .is_empty()
        );
    }

    #[test]
    fn a_show_file_holds_a_programmer_and_does_not_save_it() {
        let mut file = ShowFile::new();
        file.show = show();
        assert!(file.programmer.state().is_empty());
        file.apply(&Command::SelectFixtures {
            ids: vec![FixtureId::new(1)],
            mode: SelectionMode::Set,
        })
        .unwrap();
        assert_eq!(file.programmer.state().selection.len(), 1);
        assert!(!file.is_dirty());
    }
}
