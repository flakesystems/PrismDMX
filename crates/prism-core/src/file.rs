//! What a `.prism` file contains: a show and the session it was left in.
//!
//! `ARCHITECTURE_SPEC.md` §4.1: "Sessions are persisted with the show file:
//! reopening a show restores the console exactly as it was saved." The two are
//! separate models on purpose — a show command and a session command are
//! validated by different appliers and travel as different deltas — but they are
//! one document on disk, and one thing for the daemon to hold.
//!
//! # This is the routing D11 describes, written down
//!
//! [`ShowFile::apply`] is the whole of it: `Command::is_session_command` decides
//! which applier a command goes to, and both answer with the same
//! [`Applied`]. **S17 requirement:** the daemon dispatches here rather than
//! matching on command variants a second time — a third copy of that list is a
//! third place to forget a new command.
//!
//! # The programmer is the third half, and it is composed rather than routed
//!
//! Five commands — `SelectFixtures`, `SetAttribute`, `ApplyPreset`,
//! `ClearProgrammer` and `StoreCue` — are decided by *two* models. The show
//! validates the half only it can see (fixture 12 is not patched, preset 4 does
//! not exist, sequence 7 is not there to store into) and answers
//! [`Effect::Programmer`]; this type carries out the rest and drops the effect,
//! so a daemon applying a command here never has to know that the work was
//! split. It also owns the two places where a programmer change reaches into
//! the *session*: the jog wheel's parameter index when the selection changes,
//! and the page state on the third press of Clear.
//!
//! # The programmer is deliberately not in the file
//!
//! [`ShowFile::programmer`] is `#[serde(skip)]`. See
//! [`crate::Programmer`] for why: it is the operator's unstored edit, it
//! overrides every playback absolutely, and it is not an unsaved change to the
//! show.
//!
//! # The journal belongs at the same door, for the same reason
//!
//! One command moves all three models — `StoreCue` writes a cue *and* moves the
//! Clear stage, `SelectFixtures` moves the programmer *and* the session's jog
//! wheel — so an undo that could only reach one of them would take a command
//! half back. [`ShowFile::apply`] is the only place that sees all three, so it
//! is where a step is recorded and where `Command::Oops` and `Command::Redo`
//! are carried out; `Effect::Undo` and `Effect::Redo` are dropped from the
//! answer exactly as [`Effect::Programmer`] is. See [`crate::Journal`] for what
//! a record holds and why it is a scope rather than a copy of the show.
//!
//! # One Save LED, two sources
//!
//! The show is dirty when it has unsaved edits (S11); the session is dirty when
//! a view has been stored (S12). The lamp on the console is the two together,
//! which is [`ShowFile::is_dirty`], and `Delta::DirtyFlag` describes exactly its
//! transitions — [`ShowFile::apply`] drops the flag delta the show applier
//! raises on its own and emits one only when the pair actually changed, because
//! an LED cannot be lit twice.
//!
//! # What is deliberately not in here
//!
//! The desk identity. [`MachineConfig`](crate::MachineConfig) holds the sACN
//! CID, it is written beside the daemon's settings, and copying a show must not
//! copy it — see that module for why. This type is what a `.prism` file holds,
//! and it has no field for a desk; the schema in [`crate::ShowStore`] has no
//! table for one either, nor for the programmer or the journal.

use prism_domain::{ClearStage, Command, Delta, NoticeLevel};
use serde::{Deserialize, Serialize};

use crate::command::{Applied, Effect};
use crate::journal::{Image, Journal, JournalError, UndoRecord};
use crate::programmer::{Programmer, ProgrammerError};
use crate::session::{SessionError, SessionState};
use crate::show::{Show, ShowError};

/// Why a command could not be applied to a show file.
///
/// One error type over all three appliers, so a caller can route a command
/// without knowing in advance which half will answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShowFileError {
    /// The show refused it.
    Show(ShowError),
    /// The session refused it.
    Session(SessionError),
    /// The programmer refused it.
    Programmer(ProgrammerError),
    /// There was nothing to undo or nothing to redo.
    Journal(JournalError),
}

impl core::fmt::Display for ShowFileError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Show(error) => error.fmt(f),
            Self::Session(error) => error.fmt(f),
            Self::Programmer(error) => error.fmt(f),
            Self::Journal(error) => error.fmt(f),
        }
    }
}

impl core::error::Error for ShowFileError {}

impl From<ShowError> for ShowFileError {
    fn from(error: ShowError) -> Self {
        Self::Show(error)
    }
}

impl From<SessionError> for ShowFileError {
    fn from(error: SessionError) -> Self {
        Self::Session(error)
    }
}

impl From<ProgrammerError> for ShowFileError {
    fn from(error: ProgrammerError) -> Self {
        Self::Programmer(error)
    }
}

impl From<JournalError> for ShowFileError {
    fn from(error: JournalError) -> Self {
        Self::Journal(error)
    }
}

/// A show and the session it is operated in.
///
/// The fields are public because the two models are edited directly by the
/// sessions that own them — S13's programmer, S15's loader, S27's patch sheet —
/// and wrapping every operation of both would be a third API to keep in step.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowFile {
    /// The show: patch, profiles, groups, presets, sequences, executors.
    pub show: Show,
    /// The session: view, windows, pages, selection. V1 has exactly one, and a
    /// multi-session daemon turns this into a map keyed by `SessionId` — which
    /// is a change to the file format, and therefore S15's.
    pub session: SessionState,
    /// The programmer: the operator's live, unstored edit.
    ///
    /// Not part of the file, and `ARCHITECTURE_SPEC.md` §4.4 puts it here
    /// rather than beside the show: "V1 has exactly one session and the
    /// programmer belongs to it". A multi-session daemon gets one programmer
    /// per session, which is the same change to this type as the session map.
    #[serde(skip)]
    pub programmer: Programmer,
    /// The Oops journal: the steps that can be taken back.
    ///
    /// Public like the three models, and readable like them — but only
    /// [`Self::apply`] can *file* a step. A record is an assertion that the
    /// state was once something, and a caller able to write one could put any
    /// state it liked into the show without passing the validation every edit
    /// otherwise passes, so everything on [`Journal`] that adds or takes an
    /// entry is `pub(crate)`. What is left to a caller is reading it and
    /// [`Journal::clear`].
    #[serde(skip)]
    pub journal: Journal,
}

impl ShowFile {
    /// An empty show in a fresh session.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether anything has changed since the last save — the Save LED.
    #[must_use]
    pub const fn is_dirty(&self) -> bool {
        self.show.is_dirty() || self.session.is_dirty()
    }

    /// Records that the file has been written to disk.
    ///
    /// Returns whether the flag actually changed, so the daemon only sends a
    /// `Delta::DirtyFlag` when there is news.
    /// [`ShowStore::save`](crate::ShowStore::save) is what calls it, and it
    /// calls it when the commit has **returned** — a write that failed leaves
    /// the lamp lit, because nothing was saved.
    pub const fn mark_saved(&mut self) -> bool {
        // Both, always: a save writes both halves, so leaving either flag
        // standing would light the lamp over a file that is on disk.
        let show = self.show.mark_saved();
        let session = self.session.mark_saved();
        show || session
    }

    /// Routes a command to the applier that owns it, applies it, and files the
    /// step it took.
    ///
    /// # Errors
    ///
    /// [`ShowFileError`] if the command was refused. No half is changed after
    /// an error, and nothing is journaled.
    pub fn apply(&mut self, command: &Command) -> Result<Applied, ShowFileError> {
        let was_dirty = self.is_dirty();
        // Before anything is written: the state a later Oops has to put back.
        // Empty unless the command is undoable — see [`Self::image`].
        let before = self.image(command);
        let mut applied = if command.is_session_command() {
            self.session.apply(command)?
        } else {
            self.show.apply(command)?
        };
        if applied.effects.contains(&Effect::Programmer) {
            // The show has decided its half and named who finishes the job.
            // That somebody is here, so the effect is carried out rather than
            // handed on.
            let finished = self.finish_programmer(command)?;
            applied.absorb(Effect::Programmer, finished);
        }
        // The same shape one layer further out: the show validates `Oops` and
        // `Redo` and names the journal, and the journal is here.
        if applied.effects.contains(&Effect::Undo) {
            let undone = self.undo()?;
            applied.absorb(Effect::Undo, undone);
        }
        if applied.effects.contains(&Effect::Redo) {
            let redone = self.redo()?;
            applied.absorb(Effect::Redo, redone);
        }
        self.record(command, before);
        // The show applier raises the flag for its own half; here the flag is
        // the pair, so its own answer is replaced by the pair's transition.
        applied
            .deltas
            .retain(|delta| !matches!(delta, Delta::DirtyFlag { .. }));
        if !was_dirty && self.is_dirty() {
            applied.deltas.push(Delta::DirtyFlag {
                unsaved_changes: true,
            });
        }
        Ok(applied)
    }

    /// The half of a programmer command the show could not finish.
    ///
    /// Order matters and is the S11 rule one level up: the fallible, *writing*
    /// step goes first. `StoreCue` builds a cue out of the programmer and puts
    /// it into the show, and a refusal there must leave the programmer exactly
    /// as it was — so the programmer's own state moves only once the show has
    /// accepted.
    fn finish_programmer(&mut self, command: &Command) -> Result<Applied, ShowFileError> {
        let mut applied = Applied::default();
        if let Command::StoreCue {
            sequence_id,
            cue_number,
        } = command
        {
            let unresolved = self.programmer.unresolved(&self.show);
            let cue = self.programmer.cue(&self.show, *sequence_id, cue_number)?;
            let ops = self.show.store_cue(*sequence_id, cue)?;
            applied.deltas.push(Delta::ShowPatch { ops });
            applied.effects.push(Effect::ReloadSequence(*sequence_id));
            if !unresolved.is_empty() {
                // S6 asked for this in as many words: a value dropped silently
                // is one an operator cannot learn about.
                let dropped: Vec<String> = unresolved
                    .iter()
                    .map(|(fixture, attribute)| format!("fixture {fixture} {attribute:?}"))
                    .collect();
                applied.deltas.push(Delta::Notice {
                    level: NoticeLevel::Warn,
                    message: format!(
                        "{} programmer value(s) the patch no longer has were not stored: {}",
                        dropped.len(),
                        dropped.join(", ")
                    ),
                });
            }
        }

        let selection_before = self.programmer.state().selection.clone();
        applied
            .deltas
            .extend(self.programmer.apply(command, &self.show)?.deltas);

        // A new selection is a new list of parameters, so the wheel goes back
        // to the first of them — S12 put the index in the session and asked
        // this session to reset it through the session's own edit.
        let mut session_ops = if self.programmer.state().selection == selection_before {
            Vec::new()
        } else {
            self.session.set_programmer_param_index(0)?
        };
        // The third press of Clear takes the page state with it
        // (`docs/DMX_MERGE.md` §3.1), and the page state is the session's.
        if matches!(command, Command::ClearProgrammer)
            && self.programmer.state().clear_stage == ClearStage::Idle
        {
            session_ops.extend(self.session.set_programmer_page(0)?);
            session_ops.extend(self.session.set_programmer_param_index(0)?);
        }
        if !session_ops.is_empty() {
            applied
                .deltas
                .push(Delta::SessionPatch { ops: session_ops });
        }
        Ok(applied)
    }

    // -- the journal ------------------------------------------------------

    /// The pieces of state this command can change — `ARCHITECTURE_SPEC.md`
    /// §6.1's "affected scope", read off the command before it is applied and
    /// again afterwards.
    ///
    /// Empty for a command that is not undoable, and that is the whole of the
    /// exclusion: a playback action produces no image, so it produces no
    /// record, so an Oops cannot reach it. `Command::is_undoable` remains the
    /// definition rather than this list —
    /// `a_command_has_a_scope_exactly_when_it_is_undoable` holds the two
    /// together over all twenty-four commands, so a new command cannot be
    /// given a scope here and left out of the list there, or the reverse.
    fn image(&self, command: &Command) -> Vec<Image> {
        match command {
            Command::PatchFixture { id, .. } | Command::UnpatchFixture { id } => {
                vec![Image::Fixture(*id, self.show.fixture(*id).cloned())]
            }
            // Two entries, because a renumber is a remove and an insert: an
            // undo has to put the fixture back where it was **and** take it off
            // the number it moved to. One image would restore half of it.
            Command::RenumberFixture { id, to } => vec![
                Image::Fixture(*id, self.show.fixture(*id).cloned()),
                Image::Fixture(*to, self.show.fixture(*to).cloned()),
            ],
            Command::EmbedFixtureType { type_id } => vec![Image::FixtureType(
                type_id.clone(),
                self.show.fixture_type(type_id).cloned(),
            )],
            Command::SelectFixtures { .. }
            | Command::SetAttribute { .. }
            | Command::ApplyPreset { .. }
            | Command::ClearProgrammer => self.programmer_image(),
            Command::StoreCue { sequence_id, .. } => {
                // A sequence that is not there is not imaged, and the record is
                // never filed either: `Show::apply` refuses the command before
                // anything is written. See [`Image::Sequence`].
                let mut images: Vec<Image> = self
                    .show
                    .sequence(*sequence_id)
                    .cloned()
                    .map(Image::Sequence)
                    .into_iter()
                    .collect();
                images.extend(self.programmer_image());
                images
            }
            // The three playback actions, the two journal commands, the save
            // and the twelve session commands — named rather than caught by a
            // wildcard, so a command added to the protocol is a compile error
            // here as well as in the three appliers.
            Command::ExecutorGo { .. }
            | Command::ExecutorOff { .. }
            | Command::SetExecutorMaster { .. }
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
            | Command::CommandLineInput { .. } => Vec::new(),
        }
    }

    /// The programmer and the session's page state, which is the pair every
    /// programmer command can move — [`Self::finish_programmer`] resets the jog
    /// wheel when the selection changes and the page on the third Clear.
    ///
    /// **The page state is in the scope even though it is session state**, and
    /// that does not contradict §6.1's exclusion of the session *commands*. The
    /// exclusion is about an undo pulling windows out from under the operator;
    /// this is the cursor into the parameters of a selection, which S13 resets
    /// precisely *because* a new selection makes the old index meaningless. An
    /// undo that put the selection back and left the wheel pointing into it
    /// would restore half a state.
    fn programmer_image(&self) -> Vec<Image> {
        vec![
            Image::Programmer(self.programmer.state().clone()),
            Image::ProgrammerPage {
                page: self.session.session().programmer_page,
                param_index: self.session.session().programmer_param_index,
            },
        ]
    }

    /// Files the step a command has just taken, if it took one.
    fn record(&mut self, command: &Command, before: Vec<Image>) {
        if before.is_empty() {
            return;
        }
        let record = UndoRecord::new(command.clone(), before, self.image(command));
        if record.is_a_step() {
            self.journal.push(record);
        }
    }

    /// Takes the newest step back.
    fn undo(&mut self) -> Result<Applied, ShowFileError> {
        let record = self
            .journal
            .take_undo()
            .ok_or(JournalError::NothingToUndo)?;
        let restored = self.restore(record.before());
        match restored {
            Ok(applied) => {
                self.journal.put_redo(record);
                Ok(applied)
            }
            // Refused, and therefore not taken: the record goes back where it
            // was so the operator can press Oops again once the reason is gone.
            Err(error) => {
                self.journal.put_undo(record);
                Err(error)
            }
        }
    }

    /// Puts the newest step that was taken back, back.
    fn redo(&mut self) -> Result<Applied, ShowFileError> {
        let record = self
            .journal
            .take_redo()
            .ok_or(JournalError::NothingToRedo)?;
        let restored = self.restore(record.after());
        match restored {
            Ok(applied) => {
                self.journal.put_undo(record);
                Ok(applied)
            }
            Err(error) => {
                self.journal.put_redo(record);
                Err(error)
            }
        }
    }

    /// Puts an image of the state back, and says what changed.
    ///
    /// **The fallible half goes first**, in its own pass: a show write is the
    /// only piece of a restore that can be refused — a cue list naming a
    /// fixture somebody has since unpatched, a fixture whose profile has been
    /// replaced — and a refusal must leave every model byte-identical rather
    /// than half walked back. Restoring the programmer cannot fail, and the two
    /// session fields are `u32`s whose projection cannot either.
    ///
    /// The ordering is belt and braces *today* and is kept for what comes next:
    /// the only record that holds a show image and a desk image at once is
    /// `StoreCue`'s, and its desk half never changes — a store leaves the
    /// selection alone, and S13 proved that a store can never meet a non-zero
    /// Clear stage. A scope that grows (S27's patch sheet, S28's cue editor)
    /// would make the order load-bearing, and finding that out by way of a
    /// half-applied undo is not a good way to find it out.
    ///
    /// An image that is already what the state holds is skipped, so an undo
    /// that moves one of the three models does not broadcast a delta about the
    /// other two, or light the Save LED over a show it did not touch.
    fn restore(&mut self, images: &[Image]) -> Result<Applied, ShowFileError> {
        let mut applied = Applied::default();
        for image in images {
            match image {
                Image::Fixture(id, fixture) => {
                    let ops = match fixture {
                        Some(fixture) => self.show.patch_fixture(fixture.clone())?,
                        None => self.show.unpatch_fixture(*id)?,
                    };
                    applied.deltas.push(Delta::ShowPatch { ops });
                    // The patch changed, whichever direction it changed in: the
                    // `MergeBody` has to be rebuilt and the publisher's frame
                    // buffers blanked exactly as they do for the command that
                    // is being taken back (S4).
                    applied.effects.push(Effect::Repatch);
                }
                Image::FixtureType(type_id, fixture_type) => {
                    let ops = match fixture_type {
                        Some(fixture_type) => self.show.embed_fixture_type(fixture_type.clone())?,
                        None => self.show.remove_fixture_type(type_id)?,
                    };
                    applied.deltas.push(Delta::ShowPatch { ops });
                    applied.effects.push(Effect::Repatch);
                }
                Image::Sequence(sequence) => {
                    let ops = self.show.store_sequence(sequence.clone())?;
                    applied.deltas.push(Delta::ShowPatch { ops });
                    applied.effects.push(Effect::ReloadSequence(sequence.id));
                }
                Image::Programmer(_) | Image::ProgrammerPage { .. } => {}
            }
        }
        // A show image is written unconditionally, and does not need to ask
        // first whether it would change anything: a record is only filed when
        // its images actually moved, and a show image is the only image a
        // command with one can have moved. The two below *are* asked, because
        // both change-detect internally and a programmer command routinely
        // moves one of them and not the other.
        for image in images {
            match image {
                Image::Programmer(state) => {
                    if self.programmer.restore(state.clone()) {
                        applied.deltas.push(Delta::ProgrammerChanged {
                            state: self.programmer.state().clone(),
                        });
                    }
                }
                Image::ProgrammerPage { page, param_index } => {
                    let mut ops = self.session.set_programmer_page(*page)?;
                    ops.extend(self.session.set_programmer_param_index(*param_index)?);
                    if !ops.is_empty() {
                        applied.deltas.push(Delta::SessionPatch { ops });
                    }
                }
                Image::Fixture(..) | Image::FixtureType(..) | Image::Sequence(_) => {}
            }
        }
        Ok(applied)
    }
}

#[cfg(test)]
mod tests {
    use super::{ShowFile, ShowFileError};
    use crate::testkit::{fixture, par_type};
    use crate::{ProgrammerError, SessionError, ShowError};
    use prism_domain::{Command, Delta, ViewId, WindowInstanceId, WindowType};

    fn file() -> ShowFile {
        let mut file = ShowFile::new();
        file.show.embed_fixture_type(par_type()).unwrap();
        file.show
            .patch_fixture(fixture(1, "generic.rgbw.par", 1, 1))
            .unwrap();
        file.session
            .open_window(WindowType::FixtureSheet, None)
            .unwrap();
        file.mark_saved();
        file
    }

    #[test]
    fn a_command_goes_to_the_applier_that_owns_it() {
        let mut file = file();
        let applied = file
            .apply(&Command::OpenWindow {
                window: WindowType::Patch,
                params: None,
            })
            .unwrap();
        assert!(matches!(
            applied.deltas.as_slice(),
            [Delta::SessionPatch { .. }]
        ));
        assert_eq!(file.session.session().open_windows.len(), 2);

        // A programmer command is decided by the show and finished here, so
        // the effect that named the finisher is gone by the time a daemon sees
        // the answer.
        let applied = file
            .apply(&Command::SelectFixtures {
                ids: vec![prism_domain::FixtureId::new(1)],
                mode: prism_domain::SelectionMode::Set,
            })
            .unwrap();
        assert!(applied.effects.is_empty());
        assert!(matches!(
            applied.deltas.as_slice(),
            [Delta::ProgrammerChanged { .. }]
        ));
        assert_eq!(file.programmer.state().selection.len(), 1);
    }

    #[test]
    fn a_refusal_from_any_half_is_one_error_type() {
        let mut file = file();
        assert_eq!(
            file.apply(&Command::CloseWindow {
                instance_id: WindowInstanceId::new(9),
            }),
            Err(ShowFileError::Session(SessionError::UnknownWindow(
                WindowInstanceId::new(9)
            )))
        );
        assert_eq!(
            file.apply(&Command::ApplyPreset {
                preset_id: prism_domain::PresetId::new(9),
            }),
            Err(ShowFileError::Show(ShowError::UnknownPreset(
                prism_domain::PresetId::new(9)
            )))
        );
        // The show has nothing to say about an empty programmer — the sequence
        // exists and the cue number is a cue number — so this refusal can only
        // come from the third half.
        file.show
            .store_sequence(crate::testkit::sequence(1, Vec::new()))
            .unwrap();
        assert_eq!(
            file.apply(&Command::StoreCue {
                sequence_id: prism_domain::SequenceId::new(1),
                cue_number: "1".to_owned(),
            }),
            Err(ShowFileError::Programmer(ProgrammerError::NothingToStore))
        );

        // All three read as themselves.
        assert_eq!(
            ShowFileError::from(ShowError::NotAShowCommand).to_string(),
            ShowError::NotAShowCommand.to_string()
        );
        assert_eq!(
            ShowFileError::from(SessionError::NotASessionCommand).to_string(),
            SessionError::NotASessionCommand.to_string()
        );
        assert_eq!(
            ShowFileError::from(ProgrammerError::NothingToStore).to_string(),
            ProgrammerError::NothingToStore.to_string()
        );
    }

    #[test]
    fn the_save_led_is_lit_once_and_cleared_once() {
        let mut file = file();
        assert!(!file.is_dirty());

        // Session first, so the show's own flag delta is the redundant one.
        let applied = file
            .apply(&Command::StoreView {
                view_id: ViewId::new(2),
                name: "Programming".to_owned(),
            })
            .unwrap();
        assert!(applied.deltas.contains(&Delta::DirtyFlag {
            unsaved_changes: true
        }));
        assert!(file.is_dirty());

        let applied = file
            .apply(&Command::PatchFixture {
                id: prism_domain::FixtureId::new(2),
                name: "Two".to_owned(),
                type_id: "generic.rgbw.par".to_owned(),
                universe: prism_domain::UniverseId::new(1),
                address: 21,
            })
            .unwrap();
        assert!(
            !applied
                .deltas
                .iter()
                .any(|delta| matches!(delta, Delta::DirtyFlag { .. })),
            "the lamp was already lit"
        );
        assert!(file.show.is_dirty());

        assert!(file.mark_saved());
        assert!(!file.is_dirty());
        assert!(!file.show.is_dirty());
        assert!(!file.session.is_dirty());
        assert!(!file.mark_saved());
    }

    #[test]
    fn the_show_half_still_lights_the_lamp_by_itself() {
        let mut file = file();
        let applied = file
            .apply(&Command::PatchFixture {
                id: prism_domain::FixtureId::new(2),
                name: "Two".to_owned(),
                type_id: "generic.rgbw.par".to_owned(),
                universe: prism_domain::UniverseId::new(1),
                address: 21,
            })
            .unwrap();
        assert!(applied.deltas.contains(&Delta::DirtyFlag {
            unsaved_changes: true
        }));
    }

    #[test]
    fn an_empty_file_is_an_empty_show_in_a_fresh_session() {
        let file = ShowFile::new();
        assert_eq!(file.show.fixtures().count(), 0);
        assert_eq!(file.session.views().count(), 1);
        assert!(!file.is_dirty());
        assert!(file.journal.is_empty());
    }

    /// `ShowFile::image` and `Command::is_undoable` are two statements of one
    /// rule — what the Oops journal reaches — and this is what holds them
    /// together.
    ///
    /// A command given a scope here but left out of the list there would be
    /// journaled while the protocol says it is not; one added there and
    /// forgotten here would be journaled as an empty record that takes nothing
    /// back. Both matches are exhaustive, so a *new* command is a compile
    /// error in both; this test is for the two of them disagreeing about a
    /// command they both already name.
    #[test]
    fn a_command_has_a_scope_exactly_when_it_is_undoable() {
        use prism_domain::{
            AttributeType, ExecutorId, FeatureGroup, FixtureId, GoDirection, ParamDirection,
            PresetId, SelectionMode, SequenceId,
        };

        let file = file();
        // docs/IPC_PROTOCOL.md §5, all twenty-three.
        let commands = [
            Command::SelectFixtures {
                ids: vec![FixtureId::new(1)],
                mode: SelectionMode::Set,
            },
            Command::SetAttribute {
                attribute: AttributeType::Red,
                value: 0,
                relative: false,
            },
            Command::ApplyPreset {
                preset_id: PresetId::new(1),
            },
            Command::ClearProgrammer,
            Command::StoreCue {
                sequence_id: SequenceId::new(1),
                cue_number: "1".to_owned(),
            },
            Command::ExecutorGo {
                executor_id: ExecutorId::new(0),
                direction: GoDirection::Next,
            },
            Command::ExecutorOff {
                executor_id: ExecutorId::new(0),
            },
            Command::SetExecutorMaster {
                executor_id: ExecutorId::new(0),
                level: 0,
            },
            Command::PatchFixture {
                id: FixtureId::new(1),
                name: String::new(),
                type_id: "generic.rgbw.par".to_owned(),
                universe: prism_domain::UniverseId::new(1),
                address: 1,
            },
            Command::Oops,
            Command::Redo,
            Command::SaveShow,
            Command::SelectView {
                view_id: ViewId::new(1),
            },
            Command::StoreView {
                view_id: ViewId::new(1),
                name: String::new(),
            },
            Command::OpenWindow {
                window: WindowType::Patch,
                params: None,
            },
            Command::CloseWindow {
                instance_id: WindowInstanceId::new(1),
            },
            Command::FocusWindow {
                instance_id: WindowInstanceId::new(1),
            },
            Command::PlaceWindow {
                instance_id: WindowInstanceId::new(1),
                x: 0.0,
                y: 0.0,
                w: 1.0,
                h: 1.0,
            },
            Command::SetExecutorPage { page: 0 },
            Command::SelectExecutor {
                executor_id: ExecutorId::new(0),
            },
            Command::SetEncoderBank {
                group: FeatureGroup::Color,
            },
            Command::SetProgrammerPage { page: 0 },
            Command::SelectProgrammerParam {
                direction: ParamDirection::Next,
            },
            Command::CommandLineInput {
                text: String::new(),
            },
        ];
        assert_eq!(commands.len(), 24);
        let mut undoable = 0;
        for command in &commands {
            assert_eq!(
                !file.image(command).is_empty(),
                command.is_undoable(),
                "{command:?}"
            );
            undoable += usize::from(command.is_undoable());
        }
        // The five programmer commands and the patch: twenty-three less the
        // three playback actions, `Oops`, `Redo`, `SaveShow` and the eleven
        // §4.4 session commands.
        assert_eq!(undoable, 6);
    }
}
