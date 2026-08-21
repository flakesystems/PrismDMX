//! `Delete`, `Copy`, `Move` and `Label` over the six numbered things a desk has
//! — S40.
//!
//! # One verb, six pools
//!
//! `prism_domain::ObjectRef` says why the protocol has four commands here rather
//! than twenty-four: S40 made the command line the interface
//! (`ARCHITECTURE_SPEC.md` §4.5) and `delete <thing> <number>` is one production
//! of its grammar. This module is the other end of that decision — one applier
//! per verb, with a match over the six pools inside it, so what *delete* means
//! for a preset sits three lines from what it means for a cue and the two can be
//! read against each other.
//!
//! Views are **not** here. §4.1 puts the view library in the session, so the
//! same four verbs are applied to a view by [`crate::SessionState`]; the split
//! is `Command::is_session_command`, which reads the target.
//!
//! # What a copy means, pool by pool
//!
//! A copy leaves the source exactly as it was and writes the destination.
//! `OverwriteMode::Merge` writes into what is there and
//! `OverwriteMode::Override` replaces it, and *what is there* is a different
//! set in each pool:
//!
//! | Pool | Merge writes | Override replaces |
//! |---|---|---|
//! | Sequence | the source's cues, by number, over a cue list that keeps its others | the whole cue list |
//! | Cue | the source's parts, by fixture and attribute; times and name stay | the parts, the times, the name and the trigger |
//! | Group | the union of the two fixture lists, in the destination's order first | the fixture list |
//! | Preset | the source's values, by fixture and attribute | the values and the pool |
//!
//! In every case the destination keeps its **own name** when it already exists
//! and takes the source's when it is being created, because a copy onto
//! something an operator has named is not a rename — that is `Label`'s.
//!
//! # What a move means, and where it is not a copy-then-delete
//!
//! Four of the five are: copy, then delete the source. Two are not, and both
//! are places where the *number* is a position on the desk rather than a name
//! for a thing:
//!
//! - **an executor swaps.** `Move Executor 1 Executor 5` puts 1 on 5 and, if 5
//!   held something, puts that on 1. A desk's faders are places, and an
//!   operator rearranging them is not throwing half of them away.
//! - **a view swaps and its number stays put** (S35's decision, kept). See
//!   [`crate::SessionState::move_view`].
//!
//! And two of the copy-then-deletes carry a **reference** with them, which is
//! the part that would be a silent fault if it were forgotten:
//!
//! - moving a sequence repoints every executor that played it, or the move
//!   would leave a fader pointing at a cue list that is gone;
//! - moving a preset repoints every `CuePart::preset_ref` that named it, or
//!   every cue stored out of that preset would stop following it — invisibly,
//!   until somebody edited the preset and watched nothing happen (S28's rule).

use prism_domain::{
    Cue, CuePart, Executor, Group, JsonPatchOp, ObjectRef, OverwriteMode, Preset, PresetValue,
    Sequence, SequenceId,
};

use crate::show::{GROUPS, PRESETS, SEQUENCES, Show, ShowError, pointer, put};

impl Show {
    /// Empties one place on the desk — `Command::Delete`.
    ///
    /// # Errors
    ///
    /// The pool's own *unknown* error, or [`ShowError::NotAShowCommand`] for a
    /// view, which is the session's.
    pub fn delete_object(&mut self, target: &ObjectRef) -> Result<Vec<JsonPatchOp>, ShowError> {
        match target {
            ObjectRef::Sequence { sequence_id } => self.remove_sequence(*sequence_id),
            ObjectRef::Cue {
                sequence_id,
                cue_number,
            } => self.remove_cue(require_sequence(*sequence_id)?, cue_number),
            ObjectRef::Group { group_id } => self.remove_group(*group_id),
            ObjectRef::Preset { preset_id } => self.remove_preset(*preset_id),
            ObjectRef::Executor { executor_id } => self.remove_executor(*executor_id),
            ObjectRef::View { .. } => Err(ShowError::NotAShowCommand),
        }
    }

    /// Names one place on the desk — `Command::Label`.
    ///
    /// An **executor** has no name of its own: a strip shows the name of the cue
    /// list on it, so this labels that, and is refused on an empty slot. That is
    /// one indirection and it is the one an operator means — the scribble strip
    /// is what they are looking at.
    ///
    /// # Errors
    ///
    /// The pool's own *unknown* error, [`ShowError::ExecutorHasNoSequence`] for
    /// an empty slot, or [`ShowError::NotAShowCommand`] for a view.
    pub fn label_object(
        &mut self,
        target: &ObjectRef,
        name: &str,
    ) -> Result<Vec<JsonPatchOp>, ShowError> {
        match target {
            ObjectRef::Sequence { sequence_id } => self.label_sequence(*sequence_id, name),
            ObjectRef::Cue {
                sequence_id,
                cue_number,
            } => self.label_cue(require_sequence(*sequence_id)?, cue_number, name),
            ObjectRef::Group { group_id } => {
                let Some(group) = self.groups_mut().get_mut(group_id) else {
                    return Err(ShowError::UnknownGroup(*group_id));
                };
                if group.name == name {
                    return Ok(Vec::new());
                }
                group.name = name.to_owned();
                let group = group.clone();
                self.mark();
                Ok(vec![put(
                    pointer(GROUPS, &group.id.to_string()),
                    &group,
                    true,
                )?])
            }
            ObjectRef::Preset { preset_id } => {
                let Some(preset) = self.presets_mut().get_mut(preset_id) else {
                    return Err(ShowError::UnknownPreset(*preset_id));
                };
                if preset.name == name {
                    return Ok(Vec::new());
                }
                preset.name = name.to_owned();
                let preset = preset.clone();
                self.mark();
                Ok(vec![put(
                    pointer(PRESETS, &preset.id.to_string()),
                    &preset,
                    true,
                )?])
            }
            ObjectRef::Executor { executor_id } => {
                let Some(executor) = self.executor(*executor_id) else {
                    return Err(ShowError::UnknownExecutor(*executor_id));
                };
                let Some(sequence_id) = executor.sequence_id else {
                    return Err(ShowError::ExecutorHasNoSequence(*executor_id));
                };
                self.label_sequence(sequence_id, name)
            }
            ObjectRef::View { .. } => Err(ShowError::NotAShowCommand),
        }
    }

    /// Copies one place on the desk onto another — `Command::Copy`.
    ///
    /// The module documentation has the table of what each pool merges. The two
    /// references must name the same kind of thing, and a copy onto **itself**
    /// is refused rather than being a no-op: an operator who typed
    /// `Copy Cue 3 Cue 3` meant something else, and silence is the wrong answer.
    ///
    /// # Errors
    ///
    /// [`ShowError::MismatchedObjects`] for a mixed pair, [`ShowError::SameObject`]
    /// for a copy onto itself, the pool's own *unknown* error for a source that
    /// is not there, or [`ShowError::NotAShowCommand`] for a view.
    pub fn copy_object(
        &mut self,
        from: &ObjectRef,
        to: &ObjectRef,
        mode: OverwriteMode,
    ) -> Result<Vec<JsonPatchOp>, ShowError> {
        self.check_pair(from, to)?;
        self.transfer(from, to, mode, Naming::Keep)
    }

    /// The body of [`Self::copy_object`] and of the four-fifths of
    /// [`Self::move_object`] that is a copy.
    ///
    /// The two differ in exactly one thing and [`Naming`] is it: a **copy** onto
    /// something that already exists leaves its name alone, because a copy is
    /// not a rename; a **move** brings the name along, because the thing itself
    /// has moved and its name is part of it.
    fn transfer(
        &mut self,
        from: &ObjectRef,
        to: &ObjectRef,
        mode: OverwriteMode,
        naming: Naming,
    ) -> Result<Vec<JsonPatchOp>, ShowError> {
        match (from, to) {
            (
                ObjectRef::Sequence {
                    sequence_id: source,
                },
                ObjectRef::Sequence {
                    sequence_id: target,
                },
            ) => self.copy_sequence(*source, *target, mode, naming),
            (
                ObjectRef::Cue {
                    sequence_id: source_list,
                    cue_number: source,
                },
                ObjectRef::Cue {
                    sequence_id: target_list,
                    cue_number: target,
                },
            ) => self.copy_cue(
                require_sequence(*source_list)?,
                source,
                require_sequence(*target_list)?,
                target,
                mode,
                naming,
            ),
            (ObjectRef::Group { group_id: source }, ObjectRef::Group { group_id: target }) => {
                let Some(from) = self.group(*source).cloned() else {
                    return Err(ShowError::UnknownGroup(*source));
                };
                let fixtures = match (self.group(*target), mode) {
                    (Some(existing), OverwriteMode::Merge) => {
                        union(&existing.fixtures, &from.fixtures)
                    }
                    _ => from.fixtures.clone(),
                };
                let name = naming.pick(
                    self.group(*target).map(|existing| existing.name.as_str()),
                    &from.name,
                );
                self.store_group(Group {
                    id: *target,
                    name,
                    fixtures,
                })
            }
            (ObjectRef::Preset { preset_id: source }, ObjectRef::Preset { preset_id: target }) => {
                let Some(from) = self.preset(*source).cloned() else {
                    return Err(ShowError::UnknownPreset(*source));
                };
                let existing = self.preset(*target).cloned();
                let values = match (&existing, mode) {
                    (Some(existing), OverwriteMode::Merge) => {
                        merge_preset_values(&existing.values, &from.values)
                    }
                    _ => from.values.clone(),
                };
                self.store_preset(Preset {
                    id: *target,
                    pool: match naming {
                        Naming::Keep => existing.as_ref().map_or(from.pool, |preset| preset.pool),
                        Naming::Carry => from.pool,
                    },
                    name: naming.pick(
                        existing.as_ref().map(|preset| preset.name.as_str()),
                        &from.name,
                    ),
                    color: match naming {
                        Naming::Keep => existing
                            .as_ref()
                            .and_then(|preset| preset.color)
                            .or(from.color),
                        Naming::Carry => from.color,
                    },
                    values,
                })
            }
            (
                ObjectRef::Executor {
                    executor_id: source,
                },
                ObjectRef::Executor {
                    executor_id: target,
                },
            ) => {
                let Some(from) = self.executor(*source).cloned() else {
                    return Err(ShowError::UnknownExecutor(*source));
                };
                // Everything but the number, and **not** the playback state: an
                // executor is a fader, four keys and a cue list, and a copy that
                // carried `is_active` would claim a slot was running because
                // another one was. That state has one author (S34's tick).
                self.store_executor(Executor {
                    id: *target,
                    is_active: false,
                    current_cue_index: None,
                    ..from
                })
            }
            _ => Err(ShowError::NotAShowCommand),
        }
    }

    /// Moves one place on the desk to another — `Command::Move`.
    ///
    /// See the module documentation: an executor swaps, and the other four are a
    /// copy followed by a delete of the source, with the references that named
    /// it brought along.
    ///
    /// # Errors
    ///
    /// As [`Self::copy_object`].
    pub fn move_object(
        &mut self,
        from: &ObjectRef,
        to: &ObjectRef,
        mode: OverwriteMode,
    ) -> Result<Vec<JsonPatchOp>, ShowError> {
        self.check_pair(from, to)?;
        if let (
            ObjectRef::Executor {
                executor_id: source,
            },
            ObjectRef::Executor {
                executor_id: target,
            },
        ) = (from, to)
        {
            return self.swap_executors(*source, *target);
        }
        // Everything fallible before anything is written — S11's rule, which is
        // what makes "a refusal changes nothing" true of a two-step edit.
        self.check_source(from)?;
        let mut ops = self.transfer(from, to, mode, Naming::Carry)?;
        ops.extend(self.repoint(from, to)?);
        ops.extend(self.delete_object(from)?);
        Ok(ops)
    }

    // -- the pieces -------------------------------------------------------

    /// Renames a cue list.
    fn label_sequence(
        &mut self,
        id: SequenceId,
        name: &str,
    ) -> Result<Vec<JsonPatchOp>, ShowError> {
        let Some(sequence) = self.sequences_mut().get_mut(&id) else {
            return Err(ShowError::UnknownSequence(id));
        };
        if sequence.name == name {
            return Ok(Vec::new());
        }
        sequence.name = name.to_owned();
        let sequence = sequence.clone();
        self.mark();
        Ok(vec![put(
            pointer(SEQUENCES, &id.to_string()),
            &sequence,
            true,
        )?])
    }

    /// Renames one cue of a cue list.
    fn label_cue(
        &mut self,
        sequence_id: SequenceId,
        cue_number: &str,
        name: &str,
    ) -> Result<Vec<JsonPatchOp>, ShowError> {
        let Some(sequence) = self.sequence(sequence_id) else {
            return Err(ShowError::UnknownSequence(sequence_id));
        };
        let wanted = cue_number.trim();
        let Some(index) = sequence.cues.iter().position(|cue| cue.number == wanted) else {
            return Err(ShowError::UnknownCue {
                sequence: sequence_id,
                number: wanted.to_owned(),
            });
        };
        if sequence.cues[index].name == name {
            return Ok(Vec::new());
        }
        let mut next = sequence.clone();
        next.cues[index].name = name.to_owned();
        let op = put(pointer(SEQUENCES, &sequence_id.to_string()), &next, true)?;
        self.sequences_mut().insert(sequence_id, next);
        self.mark();
        Ok(vec![op])
    }

    /// Copies a whole cue list onto another number.
    fn copy_sequence(
        &mut self,
        source: SequenceId,
        target: SequenceId,
        mode: OverwriteMode,
        naming: Naming,
    ) -> Result<Vec<JsonPatchOp>, ShowError> {
        let Some(from) = self.sequence(source).cloned() else {
            return Err(ShowError::UnknownSequence(source));
        };
        let existing = self.sequence(target).cloned();
        let cues = match (&existing, mode) {
            (Some(existing), OverwriteMode::Merge) => merge_cues(&existing.cues, &from.cues),
            _ => from.cues.clone(),
        };
        self.store_sequence(Sequence {
            id: target,
            name: naming.pick(
                existing.as_ref().map(|sequence| sequence.name.as_str()),
                &from.name,
            ),
            cues,
            looping: match naming {
                Naming::Keep => existing.as_ref().map_or(from.looping, |held| held.looping),
                Naming::Carry => from.looping,
            },
            // Playback state has one author and it is the tick (S34).
            is_active: false,
            current_cue_index: None,
        })
    }

    /// Copies one cue onto another number, in the same cue list or another.
    fn copy_cue(
        &mut self,
        source_list: SequenceId,
        source: &str,
        target_list: SequenceId,
        target: &str,
        mode: OverwriteMode,
        naming: Naming,
    ) -> Result<Vec<JsonPatchOp>, ShowError> {
        let Some(cue) = self.cue(source_list, source).cloned() else {
            if self.sequence(source_list).is_none() {
                return Err(ShowError::UnknownSequence(source_list));
            }
            return Err(ShowError::UnknownCue {
                sequence: source_list,
                number: source.trim().to_owned(),
            });
        };
        let wanted = target.trim();
        if wanted.is_empty() {
            return Err(ShowError::EmptyCueNumber);
        }
        let existing = self.cue(target_list, wanted).cloned();
        let next = match (existing, mode) {
            // A merge keeps everything about the destination cue except the
            // values the source brings: its name, its times and its trigger are
            // the operator's and were not what they asked to copy.
            (Some(existing), OverwriteMode::Merge) => Cue {
                parts: merge_parts(&existing.parts, &cue.parts),
                ..existing
            },
            (existing, _) => Cue {
                number: wanted.to_owned(),
                name: naming.pick(existing.as_ref().map(|old| old.name.as_str()), &cue.name),
                ..cue
            },
        };
        self.store_cue(target_list, next)
    }

    /// Exchanges two executor slots, contents and all.
    fn swap_executors(
        &mut self,
        source: prism_domain::ExecutorId,
        target: prism_domain::ExecutorId,
    ) -> Result<Vec<JsonPatchOp>, ShowError> {
        let Some(from) = self.executor(source).cloned() else {
            return Err(ShowError::UnknownExecutor(source));
        };
        if source == target {
            return Err(ShowError::SameObject(format!("executor {source}")));
        }
        let held = self.executor(target).cloned();
        let mut ops = self.store_executor(Executor {
            id: target,
            is_active: false,
            current_cue_index: None,
            ..from
        })?;
        ops.extend(match held {
            Some(held) => self.store_executor(Executor {
                id: source,
                is_active: false,
                current_cue_index: None,
                ..held
            })?,
            // Nothing came back the other way, so the slot the operator moved
            // out of is empty — which is `Delete Executor`'s answer, and the
            // *place* survives because a place is arithmetic (**D7**).
            None => self.remove_executor(source)?,
        });
        Ok(ops)
    }

    /// Brings the references that named the source along to the destination.
    ///
    /// Two of them exist and both would be a silent fault — see the module
    /// documentation.
    fn repoint(&mut self, from: &ObjectRef, to: &ObjectRef) -> Result<Vec<JsonPatchOp>, ShowError> {
        match (from, to) {
            (
                ObjectRef::Sequence {
                    sequence_id: source,
                },
                ObjectRef::Sequence {
                    sequence_id: target,
                },
            ) => {
                let moved: Vec<prism_domain::ExecutorId> = self
                    .executors()
                    .filter(|executor| executor.sequence_id == Some(*source))
                    .map(|executor| executor.id)
                    .collect();
                let mut ops = Vec::new();
                for executor in moved {
                    ops.extend(self.assign_executor(executor, Some(*target))?);
                }
                Ok(ops)
            }
            (ObjectRef::Preset { preset_id: source }, ObjectRef::Preset { preset_id: target }) => {
                let mut ops = Vec::new();
                let affected: Vec<SequenceId> = self.sequences_using_preset(*source);
                for id in affected {
                    let Some(sequence) = self.sequence(id).cloned() else {
                        continue;
                    };
                    let mut next = sequence;
                    for cue in &mut next.cues {
                        for part in &mut cue.parts {
                            if part.preset_ref == Some(*source) {
                                part.preset_ref = Some(*target);
                            }
                        }
                    }
                    let op = put(pointer(SEQUENCES, &id.to_string()), &next, true)?;
                    self.sequences_mut().insert(id, next);
                    ops.push(op);
                }
                if !ops.is_empty() {
                    self.mark();
                }
                Ok(ops)
            }
            _ => Ok(Vec::new()),
        }
    }

    /// Rejects a pair that names two different kinds of thing, or one thing
    /// twice.
    fn check_pair(&self, from: &ObjectRef, to: &ObjectRef) -> Result<(), ShowError> {
        if !from.same_kind_as(to) {
            return Err(ShowError::MismatchedObjects {
                from: from.to_string(),
                to: to.to_string(),
            });
        }
        if from == to {
            return Err(ShowError::SameObject(from.to_string()));
        }
        Ok(())
    }

    /// Rejects a source that is not there, before anything has been written.
    fn check_source(&self, from: &ObjectRef) -> Result<(), ShowError> {
        match from {
            ObjectRef::Sequence { sequence_id } => {
                self.sequence(*sequence_id)
                    .ok_or(ShowError::UnknownSequence(*sequence_id))?;
            }
            ObjectRef::Cue {
                sequence_id,
                cue_number,
            } => {
                let sequence = require_sequence(*sequence_id)?;
                self.cue(sequence, cue_number)
                    .ok_or_else(|| ShowError::UnknownCue {
                        sequence,
                        number: cue_number.trim().to_owned(),
                    })?;
            }
            ObjectRef::Group { group_id } => {
                self.group(*group_id)
                    .ok_or(ShowError::UnknownGroup(*group_id))?;
            }
            ObjectRef::Preset { preset_id } => {
                self.preset(*preset_id)
                    .ok_or(ShowError::UnknownPreset(*preset_id))?;
            }
            ObjectRef::Executor { executor_id } => {
                self.executor(*executor_id)
                    .ok_or(ShowError::UnknownExecutor(*executor_id))?;
            }
            ObjectRef::View { .. } => return Err(ShowError::NotAShowCommand),
        }
        Ok(())
    }
}

/// Whether the destination keeps its own name or takes the source's.
///
/// The one difference between a copy and a move — see [`Show::transfer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Naming {
    /// A **copy**: a destination that already exists keeps the name it has.
    Keep,
    /// A **move**: the name travels with the thing, because it is part of it.
    Carry,
}

impl Naming {
    /// The name the destination ends up with.
    fn pick(self, existing: Option<&str>, source: &str) -> String {
        match (self, existing) {
            (Self::Keep, Some(name)) => name.to_owned(),
            _ => source.to_owned(),
        }
    }
}

/// The sequence a cue reference names, or the refusal for one that names none.
///
/// `ObjectRef::Cue { sequence_id: None }` means *the selected cue list*, and it
/// is resolved by [`crate::ShowFile`] before the show ever sees it — the session
/// is where the selection lives. Reaching this is therefore a caller who applied
/// a command straight to a bare [`Show`], which has no session to ask.
fn require_sequence(sequence_id: Option<SequenceId>) -> Result<SequenceId, ShowError> {
    sequence_id.ok_or(ShowError::NoSelectedSequence)
}

/// The destination's fixtures, then the source's, each once.
fn union(
    existing: &[prism_domain::FixtureId],
    incoming: &[prism_domain::FixtureId],
) -> Vec<prism_domain::FixtureId> {
    let mut out = existing.to_vec();
    for fixture in incoming {
        if !out.contains(fixture) {
            out.push(*fixture);
        }
    }
    out
}

/// The destination's cues with the source's written over them by number.
fn merge_cues(existing: &[Cue], incoming: &[Cue]) -> Vec<Cue> {
    let mut out = existing.to_vec();
    for cue in incoming {
        match out.iter_mut().find(|held| held.number == cue.number) {
            Some(held) => *held = cue.clone(),
            None => out.push(cue.clone()),
        }
    }
    out
}

/// The destination's parts with the source's written over them, by fixture and
/// attribute.
fn merge_parts(existing: &[CuePart], incoming: &[CuePart]) -> Vec<CuePart> {
    let mut out = existing.to_vec();
    for part in incoming {
        match out
            .iter_mut()
            .find(|held| held.fixture == part.fixture && held.attribute == part.attribute)
        {
            Some(held) => *held = part.clone(),
            None => out.push(part.clone()),
        }
    }
    out
}

/// The same merge one pool along.
fn merge_preset_values(existing: &[PresetValue], incoming: &[PresetValue]) -> Vec<PresetValue> {
    let mut out = existing.to_vec();
    for value in incoming {
        match out
            .iter_mut()
            .find(|held| held.fixture == value.fixture && held.attribute == value.attribute)
        {
            Some(held) => *held = value.clone(),
            None => out.push(value.clone()),
        }
    }
    out
}
