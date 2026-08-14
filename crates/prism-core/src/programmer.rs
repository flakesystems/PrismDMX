//! The programmer: what the operator has touched but not yet stored.
//!
//! `docs/DMX_MERGE.md` §3. This is the third applier in the crate, and it has
//! the shape of the other two — [`Show::apply`](crate::Show::apply) and
//! [`SessionState::apply`](crate::SessionState::apply): it validates, it
//! applies, it answers with the delta that describes what changed, and a
//! rejection leaves the state byte-identical. All three matches name every one
//! of the 23 commands without a wildcard, so a command added to the protocol is
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
    PresetId, ProgrammerState, ProgrammerValue, ProgrammerValueSource, SelectionMode, SequenceId,
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
    /// A sequence number that does not exist.
    UnknownSequence(SequenceId),
    /// An absolute attribute value outside `0..=65535`.
    ValueOutOfRange(i32),
    /// A store with nothing in the programmer to store.
    NothingToStore,
    /// A command that is not one of the five the programmer owns.
    NotAProgrammerCommand,
}

impl fmt::Display for ProgrammerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFixture(id) => write!(f, "fixture {id} is not patched"),
            Self::UnknownPreset(id) => write!(f, "there is no preset {id}"),
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

    /// The cue this programmer stores into a sequence.
    ///
    /// **Merged, not overwritten.** The programmer is sparse by specification,
    /// so a store carries only what was touched this time; overwriting would
    /// delete every value in the cue the operator did not happen to touch,
    /// which is data loss the command has no way to ask for. An existing cue
    /// keeps its name, its times and its trigger — a store is about the look.
    /// **S28 requirement:** Merge / Overwrite / Remove is a real distinction on
    /// a console, and offering it means adding a mode to `StoreCue` in
    /// `docs/IPC_PROTOCOL.md` §5.
    ///
    /// Values the show can no longer resolve are left out rather than refused;
    /// see [`Self::unresolved`]. A `presetRef` naming a preset that has since
    /// been deleted is dropped and the value kept, which is the rule S11
    /// already applied to `Show::remove_preset`.
    ///
    /// # Errors
    ///
    /// [`ProgrammerError::UnknownSequence`] if there is no such sequence, or
    /// [`ProgrammerError::NothingToStore`] if nothing would be written.
    pub fn cue(
        &self,
        show: &Show,
        sequence_id: SequenceId,
        number: &str,
    ) -> Result<Cue, ProgrammerError> {
        let Some(sequence) = show.sequence(sequence_id) else {
            return Err(ProgrammerError::UnknownSequence(sequence_id));
        };
        let parts = self.cue_parts(show);
        if parts.is_empty() {
            return Err(ProgrammerError::NothingToStore);
        }

        // The operator typed the number, so `" 2 "` and `"2"` are the same cue:
        // two cues that both read as 2 would sort equally (`Cue::compare_numbers`
        // trims) and a Goto could not say which one it meant.
        let number = number.trim();
        let mut cue = sequence
            .cues
            .iter()
            .find(|cue| cue.number == number)
            .cloned()
            .unwrap_or_else(|| Cue {
                number: number.to_owned(),
                name: String::new(),
                fade_in: DEFAULT_FADE_SECONDS,
                fade_out: DEFAULT_FADE_SECONDS,
                delay: 0.0,
                trigger: CueTrigger::Go,
                trigger_time: None,
                parts: Vec::new(),
            });
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

    /// The touched values as cue parts, in fixture then attribute order.
    fn cue_parts(&self, show: &Show) -> Vec<CuePart> {
        self.state
            .values
            .iter()
            .flat_map(|(&fixture, attributes)| {
                attributes
                    .iter()
                    .map(move |(&attribute, value)| (fixture, attribute, value))
            })
            .filter(|&(fixture, attribute, _)| show.attribute_def(fixture, attribute).is_some())
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
            Command::ApplyPreset { preset_id } => self.apply_preset(*preset_id, show)?,
            Command::ClearProgrammer => self.clear(),
            Command::StoreCue {
                sequence_id,
                cue_number,
            } => {
                // Validated even though the cue is written elsewhere, so that a
                // direct caller cannot get a bare stage reset out of a store
                // that could not have happened.
                self.cue(show, *sequence_id, cue_number)?;
                self.touch()
            }
            // The other nineteen commands, named rather than caught by a
            // wildcard: this match is then exhaustive, so a command added to
            // the protocol is a compile error here as well as in `Show::apply`
            // and `SessionState::apply`.
            Command::ExecutorGo { .. }
            | Command::ExecutorOff { .. }
            | Command::SetExecutorMaster { .. }
            | Command::PatchFixture { .. }
            | Command::UnpatchFixture { .. }
            | Command::RenumberFixture { .. }
            | Command::EmbedFixtureType { .. }
            | Command::Oops
            | Command::Redo
            | Command::SaveShow
            | Command::SelectView { .. }
            | Command::StoreView { .. }
            | Command::OpenWindow { .. }
            | Command::CloseWindow { .. }
            | Command::FocusWindow { .. }
            | Command::PlaceWindow { .. }
            | Command::SetExecutorPage { .. }
            | Command::SelectExecutor { .. }
            | Command::SetEncoderBank { .. }
            | Command::SetProgrammerPage { .. }
            | Command::SelectProgrammerParam { .. }
            | Command::CommandLineInput { .. } => {
                return Err(ProgrammerError::NotAProgrammerCommand);
            }
        };
        Ok(if changed {
            Applied {
                deltas: vec![Delta::ProgrammerChanged {
                    state: self.state.clone(),
                }],
                effects: Vec::new(),
            }
        } else {
            Applied::default()
        })
    }

    // -- edits ------------------------------------------------------------

    /// Changes the selection.
    ///
    /// `Set` replaces it, `Add` appends what is not already selected, and
    /// `Toggle` removes what is and appends what is not. The selection is a
    /// list in selection order, and no fixture appears in it twice.
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
            next.selection.clear();
        }
        for &id in ids {
            match next.selection.iter().position(|&selected| selected == id) {
                Some(position) if mode == SelectionMode::Toggle => {
                    next.selection.remove(position);
                }
                Some(_) => {}
                None => next.selection.push(id),
            }
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

    /// Advances the three-stage Clear (`docs/DMX_MERGE.md` §3.1).
    ///
    /// | Stage | Action |
    /// |---|---|
    /// | 0 → 1 | Clear the values, keep the selection |
    /// | 1 → 2 | Clear the selection |
    /// | 2 → 0 | Clear everything, including the active feature group |
    ///
    /// The stage belongs to the button rather than to the contents, so a Clear
    /// on an empty programmer still advances it. It is a cycle: the press after
    /// the third starts again at the first.
    ///
    /// **The page state is the session's half of the third stage** and
    /// [`ShowFile::apply`](crate::ShowFile::apply) resets it, because
    /// `programmerPage` and `programmerParamIndex` live in
    /// `ARCHITECTURE_SPEC.md` §4.1 rather than here.
    ///
    /// Returns whether anything changed, which for this operation is always
    /// true — the stage itself has moved.
    pub fn clear(&mut self) -> bool {
        let mut next = self.state.clone();
        match self.state.clear_stage {
            ClearStage::Idle => {
                next.values.clear();
                next.clear_stage = ClearStage::ValuesCleared;
            }
            ClearStage::ValuesCleared => {
                next.selection.clear();
                next.clear_stage = ClearStage::SelectionCleared;
            }
            ClearStage::SelectionCleared => next = ProgrammerState::default(),
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
    /// `docs/DMX_MERGE.md` §3.1: "the stage resets to 0 on any other programmer
    /// interaction, so an operator who clears once and then grabs a fader does
    /// not find a later Clear press in an unexpected stage." Every edit but
    /// [`Self::clear`] and [`Self::restore`] goes through here, which is what
    /// makes that one rule rather than four copies of it.
    fn interaction(&self) -> ProgrammerState {
        ProgrammerState {
            clear_stage: ClearStage::Idle,
            ..self.state.clone()
        }
    }

    /// An interaction that changes nothing but the Clear stage.
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
    fn commit(&mut self, next: ProgrammerState) -> bool {
        if next == self.state {
            return false;
        }
        self.state = next;
        true
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
        AttributeType, ClearStage, Command, Delta, FeatureGroup, FixtureId, Preset, PresetId,
        PresetValue, ProgrammerState, ProgrammerValue, ProgrammerValueSource, SelectionMode,
        SequenceId,
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
            pool: FeatureGroup::Color,
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

    #[test]
    fn restoring_puts_back_the_clear_stage_as_well() {
        // S14 undoes a Clear by restoring what was there, and "what was there"
        // includes which stage the button had reached.
        let show = show();
        let mut programmer = programmer(&show);
        programmer.clear();
        let stored = programmer.state().clone();
        assert_eq!(stored.clear_stage, ClearStage::ValuesCleared);

        programmer
            .select_fixtures(&[FixtureId::new(3)], SelectionMode::Set, &show)
            .unwrap();
        assert_eq!(programmer.state().clear_stage, ClearStage::Idle);

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

        let cue = programmer.cue(&show, SequenceId::new(1), "2").unwrap();
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
        let cue = programmer.cue(&show, SequenceId::new(1), " 2 ").unwrap();
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
            programmer.cue(&show, SequenceId::new(9), "1"),
            Err(ProgrammerError::UnknownSequence(SequenceId::new(9)))
        );
        assert_eq!(
            Programmer::new().cue(&show, SequenceId::new(1), "1"),
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
        let cue = programmer.cue(&show, SequenceId::new(1), "2").unwrap();
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
