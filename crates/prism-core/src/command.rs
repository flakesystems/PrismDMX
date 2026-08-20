//! Command validation and application — `docs/IPC_PROTOCOL.md` §5.
//!
//! A command expresses intent, and the daemon decides. [`Show::apply`] is where
//! the show half of that decision is made: every command in the show group is
//! either applied — with the deltas that describe what changed and the effects
//! the layers above have to carry out — or rejected, and a rejection leaves the
//! show exactly as it was.
//!
//! # What the show model can and cannot finish
//!
//! Five of the twelve show commands are the operator's *programmer*:
//! `SelectFixtures`, `SetAttribute`, `ApplyPreset`, `ClearProgrammer` and
//! `StoreCue`. The show model still has something to say about all five, and it
//! is the half that only the show knows — that fixture 12 is not patched, that
//! preset 4 does not exist, that sequence 7 is not there to store into. It
//! validates that half, changes nothing, and answers [`Effect::Programmer`].
//! [`crate::Programmer`] holds the other half, and
//! [`ShowFile::apply`](crate::ShowFile::apply) is where the two are composed —
//! so a daemon applying a command through the file never sees the seam.
//!
//! `Oops`, `Redo` and `SaveShow` are the same shape one layer further out. The
//! journal lives on [`ShowFile`](crate::ShowFile), because taking a command
//! back means taking it back in all three models at once; persistence is S15.
//! Both are validated as far as the show can see them and answered with an
//! effect.
//!
//! The three playback commands are not show state at all — an executor running
//! is the engine's business — so they validate against the show and answer with
//! the effect the daemon turns into a `prism_engine::TickCommand`. S5 recorded
//! the requirement they exist for: a daemon must play an executor **through
//! `MergeBody`** rather than reaching into the `PlaybackLayer`, because an
//! executor with a cue list on it is played, not poked.
//!
//! # Session commands are refused here on purpose
//!
//! `ARCHITECTURE_SPEC.md` §4.4's commands act on session state, which is S12.
//! `prism_domain::Command::is_session_command` is the predicate a daemon routes
//! on; [`ShowError::NotAShowCommand`] is what happens when one arrives here
//! anyway, and it changes nothing.

use prism_domain::{
    Command, Delta, ExecutorButtonFunction, ExecutorButtonRef, ExecutorFaderFunction, ExecutorId,
    Fixture, FixtureId, GoDirection, JsonPatchOp, NoticeLevel, SequenceId, Vec3,
};

use crate::show::{Show, ShowError};

/// Something the show model has decided but cannot itself carry out.
///
/// The daemon (S17) is what turns these into engine commands, journal entries
/// and disk writes. They are returned rather than performed because the show
/// model has no engine, no journal and no file — and because a command that
/// was validated but not yet executed is exactly what a caller needs in order
/// to execute it in the right order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// The patch or the embedded profiles changed.
    ///
    /// The `MergeBody` has to be rebuilt from the new patch **and the
    /// publisher's frame buffers blanked** — the encoder writes only the
    /// channels the patch covers, so a channel that is no longer patched would
    /// otherwise keep a stale value for as long as that buffer lives (S4). Any
    /// queued programmer command is stale as well, because the programmer is
    /// addressed by merge-plan slot (S6): watch [`Show::patch_revision`].
    Repatch,
    /// The groups changed: `MergeBody::load_groups` again.
    ///
    /// No command in `docs/IPC_PROTOCOL.md` §5 stores a group, so [`Show::apply`]
    /// never produces this today. It is what S27's group editor answers with
    /// when it calls [`Show::store_group`] directly — named here because the
    /// engine-side consequence of that edit belongs beside the others, not in a
    /// comment in the daemon.
    ReloadGroups,
    /// A sequence changed: `MergeBody::load_sequence` again for every executor
    /// playing it.
    ///
    /// `StoreCue` is validated here and finished by
    /// [`ShowFile::apply`](crate::ShowFile::apply), which builds the cue out of
    /// the programmer, calls [`Show::store_cue`] and answers with this.
    ReloadSequence(SequenceId),
    /// Step an executor.
    ExecutorGo {
        /// The executor.
        executor: ExecutorId,
        /// Which way.
        direction: GoDirection,
    },
    /// Stop an executor.
    ExecutorOff {
        /// The executor.
        executor: ExecutorId,
    },
    /// Start an executor at the first cue of its list.
    ///
    /// `ExecutorButtonFunction::On` (S34). Distinct from [`Self::ExecutorGo`]
    /// because a second press must not step a list that is already running —
    /// `prism_engine::CuePlayer::on` is where that rule lives, and it is the
    /// same rule `PlaybackLayer::activate` follows for the LTP order.
    ExecutorOn {
        /// The executor.
        executor: ExecutorId,
    },
    /// Hold or release a flash over an executor's master.
    ///
    /// `ExecutorButtonFunction::Flash` (S34). **Not a master move**: the stored
    /// master is untouched, so a release restores it byte for byte, and one that
    /// arrived while the flash was held is the one that stands.
    ExecutorFlash {
        /// The executor.
        executor: ExecutorId,
        /// Held or released.
        on: bool,
    },
    /// Move an executor's speed master — `docs/DMX_MERGE.md` §4 item 3.
    ExecutorSpeed {
        /// The executor.
        executor: ExecutorId,
        /// The new rate, in units of `prism_domain::SPEED_UNITY`.
        speed: u16,
    },
    /// Tap an executor's speed — `ExecutorButtonFunction::LearnSpeed`, and the
    /// *tap for speed* `docs/MCU_MAPPING.md` §4.3 recorded as having no target.
    ///
    /// The rate this learns is the engine's answer rather than the show's: two
    /// taps mean a duration, and what that means as a rate depends on the cue
    /// that is running.
    ExecutorTapSpeed {
        /// The executor.
        executor: ExecutorId,
    },
    /// Move an executor's manual crossfade — `ExecutorFaderFunction::XFade`.
    ///
    /// Carries no show state at all: where a crossfade fader stands is a
    /// gesture in progress, like a flash, and a show file that remembered one
    /// would reload holding half a cue.
    ExecutorXFade {
        /// The executor.
        executor: ExecutorId,
        /// Where the fader is, `0..=65535`.
        position: u16,
    },
    /// Move an executor's master.
    SetExecutorMaster {
        /// The executor.
        executor: ExecutorId,
        /// The new level.
        level: u16,
    },
    /// A profile out of the desk's library has to be copied into the show, and
    /// the show model has no library — the same shape as [`Self::Save`], which
    /// needs a disk.
    ///
    /// [`ShowFile::apply`](crate::ShowFile::apply) carries it out and removes it
    /// from the answer, because that is the layer S44 gave a
    /// [`crate::FixtureLibrary`] to. A caller applying `EmbedFixtureType` to a
    /// bare [`Show`] therefore gets this effect and an unchanged show: the show
    /// model cannot know whether a key names a profile, only that a profile is
    /// wanted.
    EmbedProfile,
    /// [`crate::Programmer`] owns the rest of this command. The show model has
    /// already checked everything it can see.
    ///
    /// [`ShowFile::apply`](crate::ShowFile::apply) carries it out and removes
    /// it from the answer, so this effect only ever reaches a caller who
    /// applied the command to a bare [`Show`].
    Programmer,
    /// Undo the last undoable command — the Oops journal.
    ///
    /// [`ShowFile::apply`](crate::ShowFile::apply) carries it out and removes
    /// it from the answer, exactly as it does [`Effect::Programmer`]: an undo
    /// has to reach the show, the session and the programmer in one step, and
    /// the file is the only thing that holds all three. This effect therefore
    /// only ever reaches a caller who applied `Oops` to a bare [`Show`], which
    /// has no journal to consult.
    Undo,
    /// Redo the last undone command — the same, in the other direction.
    Redo,
    /// Write the show to disk.
    ///
    /// Unlike [`Effect::Programmer`], [`Effect::Undo`] and [`Effect::Redo`],
    /// this one is **not** carried out by
    /// [`ShowFile::apply`](crate::ShowFile::apply) and reaches the daemon: a
    /// show file has a path, a disk and a failure mode, and the model that
    /// decides what a show *is* deliberately holds none of the three. The
    /// answer to it is [`ShowStore::save`](crate::ShowStore::save), which marks
    /// the file saved when the commit has returned and not before.
    Save,
}

/// What applying a command produced.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Applied {
    /// What to broadcast to clients, in order.
    pub deltas: Vec<Delta>,
    /// What the layers above still have to do.
    pub effects: Vec<Effect>,
}

impl Applied {
    /// An outcome that only asks somebody else to do something.
    fn effect(effect: Effect) -> Self {
        Self {
            deltas: Vec::new(),
            effects: vec![effect],
        }
    }

    /// Drops an effect that has just been carried out and merges what carrying
    /// it out produced.
    ///
    /// The three effects [`ShowFile::apply`](crate::ShowFile::apply) answers
    /// itself — [`Effect::Programmer`], [`Effect::Undo`] and [`Effect::Redo`] —
    /// leave the same way: a caller applying a command through the file never
    /// learns that two models decided it.
    pub(crate) fn absorb(&mut self, effect: Effect, finished: Self) {
        self.effects.retain(|found| *found != effect);
        self.deltas.extend(finished.deltas);
        self.effects.extend(finished.effects);
    }
}

impl Show {
    /// Validates a command and applies the part of it that is show state.
    ///
    /// # Errors
    ///
    /// [`ShowError`] if the command cannot be applied. **The show is unchanged
    /// after any error** — validation happens before anything is written, and
    /// `tests/command_application.rs` asserts it on the serialised bytes rather
    /// than on the claim.
    pub fn apply(&mut self, command: &Command) -> Result<Applied, ShowError> {
        let was_dirty = self.is_dirty();
        let mut applied = self.dispatch(command)?;
        if !was_dirty && self.is_dirty() {
            applied.deltas.push(Delta::DirtyFlag {
                unsaved_changes: true,
            });
        }
        Ok(applied)
    }

    /// The command itself, without the dirty-flag bookkeeping around it.
    fn dispatch(&mut self, command: &Command) -> Result<Applied, ShowError> {
        match command {
            Command::SelectFixtures { ids, .. } => {
                for id in ids {
                    self.require_fixture(*id)?;
                }
                Ok(Applied::effect(Effect::Programmer))
            }
            Command::SetAttribute {
                value, relative, ..
            } => {
                // Signed because an encoder turns both ways (S1). As an
                // absolute value it still has to be an attribute value.
                if !relative && u16::try_from(*value).is_err() {
                    return Err(ShowError::ValueOutOfRange(*value));
                }
                Ok(Applied::effect(Effect::Programmer))
            }
            Command::ApplyPreset { preset_id } => {
                if self.preset(*preset_id).is_none() {
                    return Err(ShowError::UnknownPreset(*preset_id));
                }
                Ok(Applied::effect(Effect::Programmer))
            }
            Command::ClearProgrammer => Ok(Applied::effect(Effect::Programmer)),
            Command::StoreCue {
                sequence_id,
                cue_number,
            } => {
                if self.sequence(*sequence_id).is_none() {
                    return Err(ShowError::UnknownSequence(*sequence_id));
                }
                if cue_number.trim().is_empty() {
                    return Err(ShowError::EmptyCueNumber);
                }
                Ok(Applied::effect(Effect::Programmer))
            }
            // The show sees the half of a preset store it can see: which pool
            // and which number. The values come out of the programmer, which
            // this model does not hold — the same split `StoreCue` has.
            Command::StorePreset { .. } => Ok(Applied::effect(Effect::Programmer)),
            Command::CreateSequence { sequence_id, name } => {
                let ops = self.create_sequence(*sequence_id, name)?;
                Ok(Applied {
                    deltas: vec![Delta::ShowPatch { ops }],
                    effects: vec![Effect::ReloadSequence(*sequence_id)],
                })
            }
            Command::SetCueProperty {
                sequence_id,
                cue_number,
                property,
            } => {
                let ops = self.set_cue_property(*sequence_id, cue_number, property)?;
                Ok(sequence_changed(*sequence_id, ops))
            }
            Command::DeleteCue {
                sequence_id,
                cue_number,
            } => {
                let ops = self.remove_cue(*sequence_id, cue_number)?;
                Ok(sequence_changed(*sequence_id, ops))
            }
            Command::AssignExecutor {
                executor_id,
                sequence_id,
            } => {
                let ops = self.assign_executor(*executor_id, *sequence_id)?;
                if ops.is_empty() {
                    return Ok(Applied::default());
                }
                Ok(Applied {
                    deltas: vec![Delta::ShowPatch { ops }],
                    // The executor grid is what the merge body binds sequences
                    // to, so a slot that gained one has to be loaded exactly as
                    // a sequence that gained a cue does.
                    effects: match sequence_id {
                        Some(sequence) => vec![Effect::ReloadSequence(*sequence)],
                        None => vec![Effect::ExecutorOff {
                            executor: *executor_id,
                        }],
                    },
                })
            }
            Command::ExecutorGo {
                executor_id,
                direction,
            } => {
                self.require_playable(*executor_id)?;
                Ok(Applied::effect(Effect::ExecutorGo {
                    executor: *executor_id,
                    direction: *direction,
                }))
            }
            Command::ExecutorOff { executor_id } => {
                self.require_playable(*executor_id)?;
                Ok(Applied::effect(Effect::ExecutorOff {
                    executor: *executor_id,
                }))
            }
            Command::ExecutorButton {
                executor_id,
                button,
                pressed,
            } => self.apply_executor_button(*executor_id, *button, *pressed),
            Command::SetExecutorMaster { executor_id, level } => {
                self.apply_executor_fader(*executor_id, *level)
            }
            Command::PatchFixture {
                id,
                name,
                type_id,
                universe,
                address,
            } => self.apply_patch_fixture(*id, name, type_id, *universe, *address),
            Command::UnpatchFixture { id } => {
                let ops = self.unpatch_fixture(*id)?;
                Ok(Applied {
                    deltas: vec![Delta::ShowPatch { ops }],
                    effects: vec![Effect::Repatch],
                })
            }
            Command::RenumberFixture { id, to } => {
                let ops = self.renumber_fixture(*id, *to)?;
                if ops.is_empty() {
                    // The number it already had. Nothing changed, so nothing is
                    // broadcast — an empty `ShowPatch` is a delta that says a
                    // client's mirror moved when it did not.
                    return Ok(Applied::default());
                }
                Ok(Applied {
                    deltas: vec![Delta::ShowPatch { ops }],
                    effects: vec![Effect::Repatch],
                })
            }
            // The library is `ShowFile`'s, so this is validated as far as the
            // show can see it — which is not far: a key is a key. See
            // [`Effect::EmbedProfile`].
            Command::EmbedFixtureType { .. } => Ok(Applied::effect(Effect::EmbedProfile)),
            Command::Oops => Ok(Applied::effect(Effect::Undo)),
            Command::Redo => Ok(Applied::effect(Effect::Redo)),
            Command::SaveShow => Ok(Applied::effect(Effect::Save)),
            // The fifteen session commands, named rather than caught by a
            // wildcard: this match is then exhaustive, so a command added to
            // the protocol is a compile error here instead of a silent
            // rejection at run time.
            Command::SelectView { .. }
            | Command::StoreView { .. }
            | Command::RenameView { .. }
            | Command::DeleteView { .. }
            | Command::MoveView { .. }
            | Command::OpenWindow { .. }
            | Command::CloseWindow { .. }
            | Command::FocusWindow { .. }
            | Command::PlaceWindow { .. }
            | Command::SetExecutorPage { .. }
            | Command::SelectExecutor { .. }
            | Command::SetEncoderBank { .. }
            | Command::SetProgrammerPage { .. }
            | Command::SelectProgrammerParam { .. }
            | Command::CommandLineInput { .. } => Err(ShowError::NotAShowCommand),
        }
    }

    /// Patches a fixture from the wire form of the command.
    ///
    /// Repatching keeps the geometry and the inverts. `PatchFixture` carries
    /// neither — S1 defined it as the five fields an operator supplies at patch
    /// time, with position, rotation and the inverts edited afterwards — so
    /// building a fresh [`Fixture`] from it would quietly move a moving head
    /// back to the origin of the 3D view and un-hang it from the ceiling every
    /// time somebody corrected its address.
    fn apply_patch_fixture(
        &mut self,
        id: FixtureId,
        name: &str,
        type_id: &str,
        universe: prism_domain::UniverseId,
        address: u16,
    ) -> Result<Applied, ShowError> {
        let (position, rotation, invert_pan, invert_tilt) =
            self.fixture(id)
                .map_or((Vec3::ZERO, Vec3::ZERO, false, false), |existing| {
                    (
                        existing.position,
                        existing.rotation,
                        existing.invert_pan,
                        existing.invert_tilt,
                    )
                });
        let ops = self.patch_fixture(Fixture {
            id,
            name: name.to_owned(),
            type_id: type_id.to_owned(),
            universe,
            address,
            position,
            rotation,
            invert_pan,
            invert_tilt,
        })?;

        let mut deltas = vec![Delta::ShowPatch { ops }];
        // "Detected and reported, not silently accepted": an overlap is legal
        // and is how a fixture is cloned, so it is a notice rather than a
        // refusal — but it is never silent.
        let overlaps: Vec<String> = self
            .conflicts()
            .into_iter()
            .filter(|conflict| conflict.first == id || conflict.second == id)
            .map(|conflict| conflict.to_string())
            .collect();
        if !overlaps.is_empty() {
            deltas.push(Delta::Notice {
                level: NoticeLevel::Warn,
                message: overlaps.join("; "),
            });
        }
        Ok(Applied {
            deltas,
            effects: vec![Effect::Repatch],
        })
    }

    /// Rejects a playback command on an executor that cannot play anything.
    ///
    /// An executor with no sequence is a fader that does nothing, and silence
    /// is the wrong answer: "the Go did nothing" is a complaint an operator
    /// cannot diagnose, where "executor 3 has no sequence" is one they can.
    /// Resolves a button press against the executor's own `button_functions`.
    ///
    /// **This is the whole of D3 for playback.** A client says *the third key of
    /// executor nine went down*; what that key does is show data, and what
    /// `Toggle` comes out as is `is_active`, which this model owns. A client
    /// that read `is_active` itself would race a second client doing the same,
    /// and the daemon would be told to do something nobody pressed —
    /// `docs/MCU_MAPPING.md` §4.2.1 and S26's mutation check.
    ///
    /// A key with nothing on it, and the release of a function that is not
    /// momentary, both change nothing and say so: they are ordinary, not
    /// refusals.
    fn apply_executor_button(
        &mut self,
        id: ExecutorId,
        button: ExecutorButtonRef,
        pressed: bool,
    ) -> Result<Applied, ShowError> {
        let Some(executor) = self.executor(id) else {
            return Err(ShowError::UnknownExecutor(id));
        };
        let function = match button {
            // A position the executor has no function for is a key with nothing
            // on it. Out of range is the same thing: a surface with more keys
            // per strip than this executor has assignments.
            ExecutorButtonRef::Slot { index } => executor
                .button_functions
                .get(usize::from(index))
                .copied()
                .unwrap_or_default(),
            ExecutorButtonRef::Function { function } => function,
        };
        let is_active = executor.is_active;
        let effect = match (function, pressed) {
            // A key with nothing on it, and the release of everything that is
            // not momentary. Neither is a refusal: **the executor said so**, and
            // that answer does not depend on whether it has a sequence — which
            // is why this arm comes before the check below rather than after it.
            (ExecutorButtonFunction::Empty, _) => return Ok(Applied::default()),
            // Momentary: the release is half of the gesture rather than a
            // second press.
            (ExecutorButtonFunction::Flash, on) => Effect::ExecutorFlash { executor: id, on },
            (_, false) => return Ok(Applied::default()),
            (ExecutorButtonFunction::GoForward, _) => Effect::ExecutorGo {
                executor: id,
                direction: GoDirection::Next,
            },
            (ExecutorButtonFunction::GoBack, _) => Effect::ExecutorGo {
                executor: id,
                direction: GoDirection::Prev,
            },
            (ExecutorButtonFunction::On, _) => Effect::ExecutorOn { executor: id },
            (ExecutorButtonFunction::Off, _) => Effect::ExecutorOff { executor: id },
            // The one line this session exists for: the desk reads `is_active`,
            // and no client may.
            (ExecutorButtonFunction::Toggle, _) if is_active => {
                Effect::ExecutorOff { executor: id }
            }
            (ExecutorButtonFunction::Toggle, _) => Effect::ExecutorOn { executor: id },
            (ExecutorButtonFunction::LearnSpeed, _) => Effect::ExecutorTapSpeed { executor: id },
        };
        // Everything that got this far plays back a sequence, so the executor
        // must have one — the same check `ExecutorGo` and `ExecutorOff` make,
        // and the same refusal.
        self.require_playable(id)?;
        Ok(Applied::effect(effect))
    }

    /// Resolves a fader move against the executor's own `fader_function`.
    ///
    /// One command for all four functions, because a surface and a screen both
    /// move *the fader* and which of the four it is belongs to the show
    /// (`docs/MCU_MAPPING.md` §4.1's main-fader row, and `ARCHITECTURE_SPEC.md`
    /// §6). `Master` and `Speed` are show state and travel as a patch; `XFade`
    /// is a gesture in progress and carries none.
    fn apply_executor_fader(&mut self, id: ExecutorId, level: u16) -> Result<Applied, ShowError> {
        let Some(executor) = self.executor(id) else {
            return Err(ShowError::UnknownExecutor(id));
        };
        match executor.fader_function {
            ExecutorFaderFunction::Master => {
                let ops = self.set_executor_master(id, level)?;
                Ok(Applied {
                    deltas: vec![Delta::ShowPatch { ops }],
                    effects: vec![Effect::SetExecutorMaster {
                        executor: id,
                        level,
                    }],
                })
            }
            ExecutorFaderFunction::Speed => {
                let ops = self.set_executor_speed(id, level)?;
                Ok(Applied {
                    deltas: vec![Delta::ShowPatch { ops }],
                    effects: vec![Effect::ExecutorSpeed {
                        executor: id,
                        speed: level,
                    }],
                })
            }
            ExecutorFaderFunction::XFade => Ok(Applied::effect(Effect::ExecutorXFade {
                executor: id,
                position: level,
            })),
            // A fader with nothing on it. Not a refusal: the executor says so.
            ExecutorFaderFunction::Empty => Ok(Applied::default()),
        }
    }

    fn require_playable(&self, id: ExecutorId) -> Result<(), ShowError> {
        let Some(executor) = self.executor(id) else {
            return Err(ShowError::UnknownExecutor(id));
        };
        if executor.sequence_id.is_none() {
            return Err(ShowError::ExecutorHasNoSequence(id));
        }
        Ok(())
    }
}

/// A sequence edit, or nothing at all when the edit changed nothing.
///
/// An empty operation list is not an empty `ShowPatch`: a delta describing a
/// document that did not move is a broadcast to every client saying nothing, and
/// S27 met the same case in `RenumberFixture`.
fn sequence_changed(sequence: SequenceId, ops: Vec<JsonPatchOp>) -> Applied {
    if ops.is_empty() {
        return Applied::default();
    }
    Applied {
        deltas: vec![Delta::ShowPatch { ops }],
        effects: vec![Effect::ReloadSequence(sequence)],
    }
}

/// The operations a `Delta::ShowPatch` in `deltas` carries, for tests and for
/// callers that want to feed a mirror without matching on the delta.
#[must_use]
pub fn show_patch_ops(deltas: &[Delta]) -> Vec<JsonPatchOp> {
    deltas
        .iter()
        .filter_map(|delta| match delta {
            Delta::ShowPatch { ops } => Some(ops.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Applied, Effect};
    use crate::testkit::{cue, executor, fixture, par_type, preset, sequence};
    use crate::{Show, ShowError};
    use prism_domain::{
        AttributeType, Command, Delta, ExecutorId, FixtureId, GoDirection, JsonPatchOp,
        NoticeLevel, PresetId, SelectionMode, SequenceId, UniverseId,
    };

    fn show() -> Show {
        let mut show = Show::new();
        show.embed_fixture_type(par_type()).unwrap();
        show.patch_fixture(fixture(1, "generic.rgbw.par", 1, 1))
            .unwrap();
        show.store_preset(preset(4, 1, AttributeType::Red, 65535))
            .unwrap();
        show.store_sequence(sequence(1, vec![cue("1", 1, AttributeType::Red, 65535)]))
            .unwrap();
        show.store_executor(executor(0, Some(1))).unwrap();
        show.store_executor(executor(1, None)).unwrap();
        show.mark_saved();
        show
    }

    fn patch_command(id: u32, universe: u32, address: u16) -> Command {
        Command::PatchFixture {
            id: FixtureId::new(id),
            name: format!("Fixture {id}"),
            type_id: "generic.rgbw.par".to_owned(),
            universe: UniverseId::new(universe),
            address,
        }
    }

    #[test]
    fn patching_reports_the_change_the_effect_and_the_dirty_flag() {
        let mut show = show();
        let applied = show.apply(&patch_command(2, 1, 21)).unwrap();
        assert_eq!(applied.effects, vec![Effect::Repatch]);
        assert!(matches!(
            applied.deltas.as_slice(),
            [
                Delta::ShowPatch { ops },
                Delta::DirtyFlag {
                    unsaved_changes: true
                }
            ] if matches!(ops.as_slice(), [JsonPatchOp::Add { path, .. }] if path == "/fixtures/2")
        ));
        assert_eq!(show.fixture(FixtureId::new(2)).unwrap().address, 21);
    }

    #[test]
    fn the_dirty_flag_is_sent_when_it_changes_and_not_on_every_edit() {
        let mut show = show();
        let first = show.apply(&patch_command(2, 1, 21)).unwrap();
        assert!(first.deltas.contains(&Delta::DirtyFlag {
            unsaved_changes: true
        }));
        let second = show.apply(&patch_command(3, 1, 41)).unwrap();
        assert!(
            !second
                .deltas
                .iter()
                .any(|delta| matches!(delta, Delta::DirtyFlag { .. }))
        );
    }

    #[test]
    fn an_overlap_is_applied_and_reported() {
        let mut show = show();
        let applied = show.apply(&patch_command(2, 1, 3)).unwrap();
        assert!(show.fixture(FixtureId::new(2)).is_some());
        assert_eq!(
            applied.deltas[1],
            Delta::Notice {
                level: NoticeLevel::Warn,
                message: "fixtures 1 and 2 share universe 1 channels 3-4; 2 wins".to_owned(),
            }
        );
    }

    #[test]
    fn repatching_keeps_the_geometry_the_command_does_not_carry() {
        let mut show = show();
        let mut hung = fixture(1, "generic.rgbw.par", 1, 1);
        hung.position = prism_domain::Vec3::new(1.5, 4.0, -2.0);
        hung.invert_tilt = true;
        show.patch_fixture(hung).unwrap();

        show.apply(&patch_command(1, 2, 100)).unwrap();
        let patched = show.fixture(FixtureId::new(1)).unwrap();
        assert_eq!(patched.address, 100);
        assert_eq!(patched.universe, UniverseId::new(2));
        assert_eq!(patched.position, prism_domain::Vec3::new(1.5, 4.0, -2.0));
        assert!(patched.invert_tilt);
    }

    #[test]
    fn a_playback_command_is_validated_and_handed_on() {
        let mut show = show();
        assert_eq!(
            show.apply(&Command::ExecutorGo {
                executor_id: ExecutorId::new(0),
                direction: GoDirection::Next,
            })
            .unwrap(),
            Applied {
                deltas: vec![],
                effects: vec![Effect::ExecutorGo {
                    executor: ExecutorId::new(0),
                    direction: GoDirection::Next,
                }],
            }
        );
        assert_eq!(
            show.apply(&Command::ExecutorOff {
                executor_id: ExecutorId::new(0),
            })
            .unwrap()
            .effects,
            vec![Effect::ExecutorOff {
                executor: ExecutorId::new(0)
            }]
        );
        // Playing an executor is not an edit.
        assert!(!show.is_dirty());
    }

    #[test]
    fn an_executor_with_nothing_on_it_says_so_rather_than_doing_nothing() {
        let mut show = show();
        assert_eq!(
            show.apply(&Command::ExecutorGo {
                executor_id: ExecutorId::new(1),
                direction: GoDirection::Next,
            }),
            Err(ShowError::ExecutorHasNoSequence(ExecutorId::new(1)))
        );
        assert_eq!(
            show.apply(&Command::ExecutorOff {
                executor_id: ExecutorId::new(9),
            }),
            Err(ShowError::UnknownExecutor(ExecutorId::new(9)))
        );
    }

    #[test]
    fn a_master_move_changes_the_show_and_reaches_the_engine() {
        let mut show = show();
        let applied = show
            .apply(&Command::SetExecutorMaster {
                executor_id: ExecutorId::new(0),
                level: 32768,
            })
            .unwrap();
        assert_eq!(
            applied.effects,
            vec![Effect::SetExecutorMaster {
                executor: ExecutorId::new(0),
                level: 32768,
            }]
        );
        assert_eq!(
            show.executor(ExecutorId::new(0)).unwrap().master_level,
            32768
        );
        assert_eq!(
            show.apply(&Command::SetExecutorMaster {
                executor_id: ExecutorId::new(9),
                level: 0,
            }),
            Err(ShowError::UnknownExecutor(ExecutorId::new(9)))
        );
    }

    #[test]
    fn the_programmer_commands_are_validated_here_and_finished_in_s13() {
        let mut show = show();
        for command in [
            Command::SelectFixtures {
                ids: vec![FixtureId::new(1)],
                mode: SelectionMode::Set,
            },
            Command::SetAttribute {
                attribute: AttributeType::Red,
                value: 65535,
                relative: false,
            },
            Command::SetAttribute {
                attribute: AttributeType::Red,
                value: -128,
                relative: true,
            },
            Command::ApplyPreset {
                preset_id: PresetId::new(4),
            },
            Command::ClearProgrammer,
            Command::StoreCue {
                sequence_id: SequenceId::new(1),
                cue_number: "2".to_owned(),
            },
        ] {
            let applied = show.apply(&command).unwrap();
            assert_eq!(applied.effects, vec![Effect::Programmer], "{command:?}");
            assert!(applied.deltas.is_empty(), "{command:?}");
        }
        assert!(!show.is_dirty());
    }

    #[test]
    fn the_show_half_of_a_programmer_command_is_checked_here() {
        let mut show = show();
        assert_eq!(
            show.apply(&Command::SelectFixtures {
                ids: vec![FixtureId::new(1), FixtureId::new(9)],
                mode: SelectionMode::Add,
            }),
            Err(ShowError::UnknownFixture(FixtureId::new(9)))
        );
        assert_eq!(
            show.apply(&Command::ApplyPreset {
                preset_id: PresetId::new(9),
            }),
            Err(ShowError::UnknownPreset(PresetId::new(9)))
        );
        assert_eq!(
            show.apply(&Command::StoreCue {
                sequence_id: SequenceId::new(9),
                cue_number: "1".to_owned(),
            }),
            Err(ShowError::UnknownSequence(SequenceId::new(9)))
        );
        assert_eq!(
            show.apply(&Command::StoreCue {
                sequence_id: SequenceId::new(1),
                cue_number: "  ".to_owned(),
            }),
            Err(ShowError::EmptyCueNumber)
        );
    }

    #[test]
    fn an_absolute_attribute_value_has_to_be_an_attribute_value() {
        let mut show = show();
        for value in [-1, 65536] {
            assert_eq!(
                show.apply(&Command::SetAttribute {
                    attribute: AttributeType::Dimmer,
                    value,
                    relative: false,
                }),
                Err(ShowError::ValueOutOfRange(value))
            );
        }
        // A relative value is a signed delta and may be either.
        show.apply(&Command::SetAttribute {
            attribute: AttributeType::Dimmer,
            value: -70000,
            relative: true,
        })
        .unwrap();
    }

    #[test]
    fn the_journal_and_the_disk_are_asked_rather_than_answered() {
        let mut show = show();
        assert_eq!(
            show.apply(&Command::Oops).unwrap().effects,
            vec![Effect::Undo]
        );
        assert_eq!(
            show.apply(&Command::Redo).unwrap().effects,
            vec![Effect::Redo]
        );
        assert_eq!(
            show.apply(&Command::SaveShow).unwrap().effects,
            vec![Effect::Save]
        );
        assert!(!show.is_dirty());
    }

    #[test]
    fn a_session_command_is_not_a_show_command() {
        let mut show = show();
        assert_eq!(
            show.apply(&Command::SelectView {
                view_id: prism_domain::ViewId::new(1),
            }),
            Err(ShowError::NotAShowCommand)
        );
    }

    #[test]
    fn show_patch_ops_collects_the_operations_out_of_a_delta_list() {
        let deltas = vec![
            Delta::ShowPatch {
                ops: vec![JsonPatchOp::Remove {
                    path: "/a".to_owned(),
                }],
            },
            Delta::DirtyFlag {
                unsaved_changes: true,
            },
        ];
        assert_eq!(
            super::show_patch_ops(&deltas),
            vec![JsonPatchOp::Remove {
                path: "/a".to_owned()
            }]
        );
    }
}
