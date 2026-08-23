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

use prism_domain::{
    AttributeType, ClearStage, Command, CueEdit, Delta, FeatureGroup, FixtureId, JsonPatchOp,
    NoticeLevel, ObjectRef, PlaybackTarget, PresetId, Sequence, SequenceId, StoreMode,
    StorePreview, StoreTarget,
};
use serde::{Deserialize, Serialize};

use crate::command::{Applied, Effect};
use crate::journal::{Image, Journal, JournalError, UndoRecord};
use crate::programmer::{Programmer, ProgrammerError};
use crate::session::{SessionError, SessionState};
use crate::show::{Show, ShowError};

/// The extension a show file has to have — S37.
const SHOW_EXTENSION: &str = ".prism";

/// The extension a JSON export has to have — S37.
const EXPORT_EXTENSION: &str = ".json";

/// Reads a `.prism` path out of a file command — S37.
///
/// # Why the check is here and why it is only this much
///
/// A path from a client is a string, and three things can be decided about one
/// without touching a disk: that it is not empty, that it names a file rather
/// than a directory, and that its extension is the one this command's format
/// actually is. Everything else — whether it exists, whether it can be written,
/// whether it is a show at all — needs the file system and is `prismd`'s.
///
/// A **relative** path is accepted and is deliberately not resolved here: a
/// client and a daemon do not share a working directory, so which directory a
/// bare `aula.prism` means is the daemon's answer (its data directory) and not
/// this crate's.
///
/// # Errors
///
/// [`ShowError::NotAShowPath`].
pub(crate) fn show_path(path: &str) -> Result<std::path::PathBuf, ShowError> {
    checked_path(path, SHOW_EXTENSION)
}

/// Reads a `.json` path out of an export or an import — S37.
///
/// # Errors
///
/// [`ShowError::NotAShowPath`].
pub(crate) fn export_path(path: &str) -> Result<std::path::PathBuf, ShowError> {
    checked_path(path, EXPORT_EXTENSION)
}

/// The shared half of the two above.
fn checked_path(path: &str, wanted: &'static str) -> Result<std::path::PathBuf, ShowError> {
    let trimmed = path.trim();
    let refuse = || ShowError::NotAShowPath {
        path: path.to_owned(),
        wanted,
    };
    if trimmed.is_empty() || !trimmed.to_ascii_lowercase().ends_with(wanted) {
        return Err(refuse());
    }
    let candidate = std::path::PathBuf::from(trimmed);
    // A file name of exactly the extension — `.prism` on its own — is a hidden
    // file on one platform and a mistake on every one of them.
    let named = candidate
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.len() > wanted.len());
    if named { Ok(candidate) } else { Err(refuse()) }
}

/// Why a command could not be applied to a show file.
///
/// One error type over all three appliers, so a caller can route a command
/// without knowing in advance which half will answer.
///
/// **Not `Eq`**, because [`ShowError`] is not: `ShowError::NegativeTime` carries
/// the number an operator typed.
#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// The profiles this **desk** can embed into the show — S44.
    ///
    /// Not part of the file, and not part of the show either: a show *embeds*
    /// the profiles it uses (S11), so what is here is where the first copy
    /// comes from and nothing more. A show saved on a desk with the whole Open
    /// Fixture Library opens unchanged on one with none of it.
    ///
    /// It is here rather than on [`Show`] because it is the same kind of thing
    /// as the journal: state about this *run*, which the file must not carry.
    /// It defaults to the four generic profiles, so a `ShowFile` built in a test
    /// can still embed something; the daemon replaces it once it has read the
    /// profile directories.
    #[serde(skip, default = "crate::FixtureLibrary::generic")]
    pub library: crate::FixtureLibrary,
}

/// A fresh file, with the desk's **built-in** profiles in it.
///
/// Written out rather than derived, because `FixtureLibrary::default()` is
/// deliberately *empty* — the daemon builds one by reading the operator's
/// folder, then the installed library, then adding the generics last, and an
/// empty start is what makes that order mean anything. A `ShowFile` on its own
/// is the other case: a test, or a daemon that has not read anything yet, and
/// neither should be unable to embed a dimmer.
impl Default for ShowFile {
    fn default() -> Self {
        Self {
            show: Show::default(),
            session: SessionState::default(),
            programmer: Programmer::default(),
            journal: Journal::default(),
            library: crate::FixtureLibrary::generic(),
        }
    }
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

    /// What storing the programmer into a cue or a preset **would** do.
    ///
    /// The answer to `Query::StorePreview`, and S28's exit criterion: *a store
    /// that would overwrite says what it will do before it does it, even where
    /// the only mode available is Merge.*
    ///
    /// It is here rather than on [`Show`] because it needs all three of the
    /// things this type holds together — what the programmer is holding, what is
    /// already filed under that number, and which store mode this build has.
    /// And it takes its refusal from the **same** builders the store runs
    /// ([`crate::Programmer::cue`] and [`crate::Programmer::preset`]), so a
    /// preview and the store after it cannot disagree — exactly as
    /// `Show::preview_patch` shares `check_patch` with the patch (S27).
    ///
    /// Writes nothing.
    #[must_use]
    pub fn preview_store(&self, target: &StoreTarget, mode: StoreMode) -> StorePreview {
        match target {
            StoreTarget::Cue {
                sequence_id,
                cue_number,
            } => self.preview_cue(*sequence_id, cue_number, mode),
            StoreTarget::Preset { preset_id, pool } => self.preview_preset(*preset_id, *pool, mode),
        }
    }

    /// [`Self::preview_store`] for a cue.
    fn preview_cue(
        &self,
        sequence_id: SequenceId,
        cue_number: &str,
        mode: StoreMode,
    ) -> StorePreview {
        let wanted = cue_number.trim();
        let existing = self
            .show
            .sequence(sequence_id)
            .and_then(|sequence| sequence.cues.iter().find(|cue| cue.number == wanted));
        let stored: Vec<(FixtureId, AttributeType)> = existing
            .map(|cue| {
                cue.parts
                    .iter()
                    .map(|part| (part.fixture, part.attribute))
                    .collect()
            })
            .unwrap_or_default();
        match self
            .programmer
            .cue(&self.show, sequence_id, cue_number, mode)
        {
            Ok(_) => counted(
                existing.map(|cue| cue.name.as_str()),
                &stored,
                &self.programmer.stored_keys(&self.show, None),
                mode,
            ),
            Err(error) => refused(
                existing.map(|cue| cue.name.as_str()),
                &error.to_string(),
                mode,
            ),
        }
    }

    /// [`Self::preview_store`] for a preset.
    fn preview_preset(
        &self,
        preset_id: PresetId,
        pool: FeatureGroup,
        mode: StoreMode,
    ) -> StorePreview {
        let existing = self.show.preset(preset_id);
        let stored: Vec<(FixtureId, AttributeType)> = existing
            .map(|preset| {
                preset
                    .values
                    .iter()
                    .map(|value| (value.fixture, value.attribute))
                    .collect()
            })
            .unwrap_or_default();
        // The name is the *client's* here — `StorePreset` carries it — so what
        // is reported is what the preset is called now, which is the fact an
        // operator is about to write over.
        match self
            .programmer
            .preset(&self.show, preset_id, pool, "", None, mode)
        {
            Ok(_) => counted(
                existing.map(|preset| preset.name.as_str()),
                &stored,
                &self.programmer.stored_keys(&self.show, Some(pool)),
                mode,
            ),
            Err(error) => refused(
                existing.map(|preset| preset.name.as_str()),
                &error.to_string(),
                mode,
            ),
        }
    }

    /// Routes a command to the applier that owns it, applies it, and files the
    /// step it took.
    ///
    /// # Errors
    ///
    /// [`ShowFileError`] if the command was refused. No half is changed after
    /// an error, and nothing is journaled.
    pub fn apply(&mut self, command: &Command) -> Result<Applied, ShowFileError> {
        // **Everything the session has to fill in, filled in first** (S40). A
        // line that names no cue list means the selected one and a playback
        // target of `Selected` means the same cue list; both are session state,
        // so the daemon resolves them and a client never does. Done here rather
        // than in either applier because this is the only type that holds the
        // session and the show at once - the same reason `follow_cue_edit` is a
        // method on this type - and done *before* the image is taken, so the
        // journal records the command that was actually carried out.
        let resolved = self.resolve(command)?;
        let command = &resolved;
        let was_dirty = self.is_dirty();
        // Before anything is written: the state a later Oops has to put back.
        // Empty unless the command is undoable — see [`Self::image`].
        let before = self.image(command);
        let mut applied = if command.is_session_command() {
            self.session.apply(command)?
        } else {
            self.show.apply(command)?
        };
        if applied.effects.contains(&Effect::EmbedProfile) {
            // The show model said a profile is wanted and could not say whether
            // it exists — it has no library. This layer does. Carried out
            // *before* the record is filed, so an Oops takes it back.
            let embedded = self.finish_embed(command)?;
            applied.absorb(Effect::EmbedProfile, embedded);
        }
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
        // After the show write and before the record is filed, so an Oops takes
        // the update state back with the edit that moved it.
        let ops = self.follow_cue_edit(command)?;
        if !ops.is_empty() {
            applied.deltas.push(Delta::SessionPatch { ops });
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

    /// Fills in what the **session** knows and the line did not say — S40.
    ///
    /// Two things, and both are `Session::selectedSequence` (`ARCHITECTURE_SPEC.md`
    /// §4.1, S39's field):
    ///
    /// - a cue named with no cue list — `Store Cue 5`, `Delete Cue 3`,
    ///   `Label Cue 3 "Blackout"`, `Copy Cue 2 Cue 6`;
    /// - `PlaybackTarget::Selected` — a bare `Go+`, `On`, `Off` or `Goto Cue 5`.
    ///
    /// A command that named everything it needs is returned unchanged, which is
    /// every command a surface sends and most of what a screen sends.
    ///
    /// # Errors
    ///
    /// [`ShowError::NoSelectedSequence`] when a line needs the selection and
    /// there is none. A message rather than a silence: *no cue list is selected*
    /// is a complaint an operator can act on.
    fn resolve(&self, command: &Command) -> Result<Command, ShowFileError> {
        let selected = || {
            self.session
                .session()
                .selected_sequence
                .ok_or(ShowFileError::Show(ShowError::NoSelectedSequence))
        };
        let cue_list = |sequence_id: Option<SequenceId>| match sequence_id {
            Some(id) => Ok(Some(id)),
            None => selected().map(Some),
        };
        let object = |target: &ObjectRef| -> Result<ObjectRef, ShowFileError> {
            match target {
                ObjectRef::Cue {
                    sequence_id,
                    cue_number,
                } => Ok(ObjectRef::Cue {
                    sequence_id: cue_list(*sequence_id)?,
                    cue_number: cue_number.clone(),
                }),
                other => Ok(other.clone()),
            }
        };
        let playback = |target: &PlaybackTarget| -> Result<PlaybackTarget, ShowFileError> {
            match target {
                PlaybackTarget::Selected => Ok(PlaybackTarget::Sequence {
                    sequence_id: selected()?,
                }),
                other => Ok(*other),
            }
        };
        Ok(match command {
            Command::StoreCue {
                sequence_id,
                cue_number,
                mode,
            } => Command::StoreCue {
                sequence_id: cue_list(*sequence_id)?,
                cue_number: cue_number.clone(),
                mode: *mode,
            },
            Command::EditCue {
                sequence_id,
                cue_number,
            } => Command::EditCue {
                sequence_id: cue_list(*sequence_id)?,
                cue_number: cue_number.clone(),
            },
            Command::SetCueProperty {
                sequence_id,
                cue_number,
                property,
            } => Command::SetCueProperty {
                sequence_id: cue_list(*sequence_id)?,
                cue_number: cue_number.clone(),
                property: property.clone(),
            },
            Command::Delete { target } => Command::Delete {
                target: object(target)?,
            },
            Command::Label { target, name } => Command::Label {
                target: object(target)?,
                name: name.clone(),
            },
            Command::Copy { from, to, mode } => Command::Copy {
                from: object(from)?,
                to: object(to)?,
                mode: *mode,
            },
            Command::Move { from, to, mode } => Command::Move {
                from: object(from)?,
                to: object(to)?,
                mode: *mode,
            },
            Command::ExecutorGo { target, direction } => Command::ExecutorGo {
                target: playback(target)?,
                direction: *direction,
            },
            Command::ExecutorOff { target } => Command::ExecutorOff {
                target: playback(target)?,
            },
            Command::ExecutorOn { target } => Command::ExecutorOn {
                target: playback(target)?,
            },
            Command::Goto { target, cue_number } => Command::Goto {
                target: playback(target)?,
                cue_number: cue_number.clone(),
            },
            // Storing a preset with no pool named means the bank the operator
            // has under their hands - `Session::encoderBank`, and
            // `Command::StorePreset` has the argument for it. A preset that
            // already exists keeps its own pool instead, which
            // `Programmer::preset` decides, because a store into preset 1 is a
            // store into preset 1 rather than a way of moving it between pools.
            Command::StorePreset {
                preset_id,
                pool,
                name,
                color,
                mode,
            } => Command::StorePreset {
                preset_id: *preset_id,
                pool: Some(pool.unwrap_or_else(|| self.session.session().encoder_bank)),
                name: name.clone(),
                color: *color,
                mode: *mode,
            },
            other => other.clone(),
        })
    }

    /// The half of `EmbedFixtureType` the show could not finish.
    ///
    /// # Errors
    ///
    /// [`ShowError::UnknownLibraryType`] when this desk carries no profile of
    /// that key. The key is the *client's* — a client sends a key and never a
    /// profile (D3) — so this is where a key that named nothing is refused, and
    /// it is refused before anything is written.
    fn finish_embed(&mut self, command: &Command) -> Result<Applied, ShowFileError> {
        let Command::EmbedFixtureType { type_id } = command else {
            return Ok(Applied::default());
        };
        let Some(profile) = self.library.profile(type_id).cloned() else {
            return Err(ShowFileError::Show(ShowError::UnknownLibraryType(
                type_id.clone(),
            )));
        };
        let ops = self.show.embed_fixture_type(profile)?;
        Ok(Applied {
            deltas: vec![Delta::ShowPatch { ops }],
            effects: vec![Effect::Repatch],
        })
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
        // `Update` is a `StoreCue` in Override mode whose target is the desk's
        // rather than the client's, so it is *resolved* into one here rather
        // than implemented twice. That is also what makes the update state's one
        // rule — an Override store into the cue being edited clears the flag —
        // true of both of them without being written down twice.
        let resolved = match command {
            Command::StoreCue {
                sequence_id,
                cue_number,
                mode,
            } => Some((
                sequence_id.ok_or(ShowFileError::Show(ShowError::NoSelectedSequence))?,
                cue_number.clone(),
                *mode,
            )),
            Command::Update => {
                let Some(editing) = self.session.session().editing_cue.clone() else {
                    return Err(ShowFileError::Programmer(
                        ProgrammerError::NothingIsBeingEdited,
                    ));
                };
                Some((editing.sequence_id, editing.cue_number, StoreMode::Override))
            }
            _ => None,
        };
        if let Some((sequence_id, cue_number, mode)) = resolved {
            let cue_number = cue_number.as_str();
            let unresolved = self.programmer.unresolved(&self.show);
            let cue = self
                .programmer
                .cue(&self.show, sequence_id, cue_number, mode)?;
            let ops = self.show.store_cue(sequence_id, cue)?;
            applied.deltas.push(Delta::ShowPatch { ops });
            applied.effects.push(Effect::ReloadSequence(sequence_id));
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

        if let Command::StoreSequence {
            sequence_id,
            name,
            mode,
        } = command
        {
            // **The cue list is made when the number is free** (S40): the
            // command line cannot know whether sequence 4 exists, because the
            // parser does not read the show (S26), so `Store Sequence 4` is both
            // acts. It is built *beside* the show and written once, so a store
            // that is refused does not leave a half-made cue list behind — S11's
            // rule, and the one this nearly broke.
            let existing = self.show.sequence(*sequence_id).cloned();
            let base = existing.clone().unwrap_or_else(|| Sequence {
                id: *sequence_id,
                name: name.clone(),
                // A store makes a cue list; colouring one is `Command::Color`,
                // for the reason a store does not rename one either.
                color: None,
                cues: Vec::new(),
                looping: false,
                is_active: false,
                current_cue_index: None,
            });
            let stored = match self.programmer.sequence(&self.show, &base, *mode) {
                Ok(sequence) => sequence,
                // **An empty programmer on a number nobody has used is the
                // whole act**, and it is what `Command::CreateSequence` used to
                // be: the cue list is made and nothing is stored into it. On a
                // list that is already there it is an ordinary refusal.
                Err(ProgrammerError::NothingToStore) if existing.is_none() => base,
                Err(error) => return Err(error.into()),
            };
            let created = existing.is_none();
            let ops = self.show.store_sequence(stored)?;
            applied.deltas.push(Delta::ShowPatch { ops });
            applied.effects.push(Effect::ReloadSequence(*sequence_id));
            // **A cue list that was just made is the one being edited.**
            //
            // Not tidiness: `Store Cue 5` names no cue list and means the
            // selected one (§4.1), so the very next line an operator types
            // after `Store Sequence 4` would otherwise go into whatever was
            // selected before — or be refused for naming nothing. A store
            // *into* a list that already existed leaves the selection alone,
            // because choosing what to edit is `Sequence 4`'s job and an
            // operator storing into a second list has not said they want to
            // move there.
            if created {
                let ops = self.session.select_sequence(Some(*sequence_id))?;
                if !ops.is_empty() {
                    applied.deltas.push(Delta::SessionPatch { ops });
                }
            }
        }

        // S40's group store: the *selection*, not the values. A group is a list
        // of fixtures (`prism_domain::Group`), which is what makes `Group 3` a
        // selection rather than a look, and it is the programmer's half in the
        // same way a cue's values are.
        if let Command::StoreGroup {
            group_id,
            name,
            mode,
        } = command
        {
            let group = self.programmer.group(&self.show, *group_id, name, *mode)?;
            let ops = self.show.store_group(group)?;
            applied.deltas.push(Delta::ShowPatch { ops });
            applied.effects.push(Effect::ReloadGroups);
        }

        if let Command::StorePreset {
            preset_id,
            pool,
            name,
            color,
            mode,
        } = command
        {
            // The same order as `StoreCue`: the fallible, *writing* step first,
            // so a refusal leaves the programmer exactly as it was.
            let preset = self.programmer.preset(
                &self.show,
                *preset_id,
                // `Self::resolve` has filled this in from the session's
                // encoder bank, so a `None` here is a caller who applied the
                // command to something that is not a `ShowFile`.
                pool.ok_or(ShowFileError::Show(ShowError::NoSelectedSequence))?,
                name,
                *color,
                *mode,
            )?;
            // Read **before** the store, because a store that changes a linked
            // value has to reload the sequence it changed — and afterwards the
            // question would be asked of a show that had already moved.
            let affected = self.show.sequences_using_preset(*preset_id);
            let ops = self.show.store_preset(preset)?;
            applied.deltas.push(Delta::ShowPatch { ops });
            applied
                .effects
                .extend(affected.into_iter().map(Effect::ReloadSequence));
        }

        let selection_before = self.programmer.state().selection.clone();
        // **A store that has already been written is not asked about again.**
        // The cue, the preset or the cue list was built above out of the show as
        // it stood *before* the write; re-deriving it through `Programmer::apply`
        // would ask the same question of the show this store has just changed,
        // and `StoreMode::Remove` answers it differently — there is nothing left
        // to remove — so the command would be refused after it had been applied.
        // See [`Programmer::stored`].
        let half = match command {
            Command::StoreCue { .. }
            | Command::StorePreset { .. }
            | Command::StoreSequence { .. }
            | Command::StoreGroup { .. }
            | Command::Update => self.programmer.stored(),
            other => self.programmer.apply(other, &self.show)?,
        };
        applied.deltas.extend(half.deltas);

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

    // -- the update state (S39) -------------------------------------------

    /// Moves `Session::editing_cue` to wherever this command has left it.
    ///
    /// **Six transitions, and every one of them is a thing that happened to the
    /// cue or to the programmer** — which is why this is a method on the file
    /// and not a rule in the session applier: only this type sees both.
    ///
    /// | Command | What becomes of the update state |
    /// |---|---|
    /// | `EditCue` | it *is* the update state: that cue, unmodified |
    /// | `Update`, and a `StoreCue` in Override mode into the cue being edited | unmodified again — the cue and the programmer now agree |
    /// | `ClearProgrammer` | **cleared**: the values it was holding are gone, whatever stage the button was in |
    /// | `Delete` of the cue being edited | **cleared**: there is nothing to put back |
    /// | `Move` of the cue being edited — a renumber | it **follows** — see below |
    /// | any other programmer edit | modified, which is what blinks the key |
    ///
    /// The last two were `DeleteCue` and `CueProperty::Number` until S40, which
    /// replaced both with the verbs every pool shares. The rules did not move
    /// with them.
    ///
    /// A renumber follows rather than clearing, and that is the one decision in
    /// here. The alternative is to clear, and the reason not to is what an
    /// `Update` would do afterwards: it stores in Override mode into the number
    /// it remembers, so a cleared-and-not-cleared mistake here would **recreate
    /// the cue at its old number** — an operator who corrects `1` to `1.5` and
    /// presses Update would end up with both. The cue an operator is editing is
    /// still that cue after they have corrected its number.
    ///
    /// # Errors
    ///
    /// [`SessionError::NotRepresentable`] only, and only by way of the session
    /// edit — a `CueEdit` is two numbers and a flag.
    fn follow_cue_edit(&mut self, command: &Command) -> Result<Vec<JsonPatchOp>, ShowFileError> {
        let editing = self.session.session().editing_cue.clone();
        let next = match command {
            Command::EditCue {
                sequence_id: Some(sequence_id),
                cue_number,
            } => Some(CueEdit {
                sequence_id: *sequence_id,
                cue_number: cue_number.trim().to_owned(),
                modified: false,
            }),
            Command::ClearProgrammer => None,
            Command::Update => editing.clone().map(|edit| CueEdit {
                modified: false,
                ..edit
            }),
            Command::StoreCue {
                sequence_id: Some(sequence_id),
                cue_number,
                mode,
            } => match editing.clone() {
                Some(edit)
                    if *mode == StoreMode::Override
                        && edit.sequence_id == *sequence_id
                        && edit.cue_number == cue_number.trim() =>
                {
                    Some(CueEdit {
                        modified: false,
                        ..edit
                    })
                }
                other => other.map(touched),
            },
            Command::Delete {
                target:
                    ObjectRef::Cue {
                        sequence_id: Some(sequence_id),
                        cue_number,
                    },
            } => editing
                .clone()
                .filter(|edit| !edit.is_of(*sequence_id, cue_number)),
            // A `Move` of a cue is a renumber, and a renumber of the cue being
            // edited **carries the edit with it** — see below. A move *onto* the
            // cue being edited replaces it, and the edit is of a cue that is
            // still there, so it stands and is marked modified by neither: it
            // was the destination rather than the programmer that changed.
            Command::Move {
                from:
                    ObjectRef::Cue {
                        sequence_id: Some(sequence_id),
                        cue_number,
                    },
                to:
                    ObjectRef::Cue {
                        sequence_id: Some(target_list),
                        cue_number: number,
                    },
                ..
            } => editing.clone().map(|edit| {
                if edit.is_of(*sequence_id, cue_number) {
                    CueEdit {
                        sequence_id: *target_list,
                        cue_number: number.trim().to_owned(),
                        ..edit
                    }
                } else {
                    edit
                }
            }),
            // Every other programmer command marks it modified — and only if it
            // actually moved the programmer, which is what `Effect::Programmer`
            // having produced a `ProgrammerChanged` already told us. Reading the
            // command rather than the delta would mark an encoder turned with
            // nothing selected as an edit.
            Command::SelectFixtures { .. }
            | Command::SelectGroup { .. }
            | Command::SetAttribute { .. }
            | Command::ApplyPreset { .. }
            | Command::StoreSequence { .. }
            | Command::StoreGroup { .. }
            | Command::StorePreset { .. } => editing.clone().map(touched),
            _ => editing.clone(),
        };
        if next == editing {
            return Ok(Vec::new());
        }
        Ok(self.session.set_editing_cue(next)?)
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
    /// together over every command that has one, so a new command cannot be
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
            | Command::SelectGroup { .. }
            | Command::SetAttribute { .. }
            | Command::ApplyPreset { .. }
            | Command::ClearProgrammer => self.programmer_image(),
            Command::StoreCue { sequence_id, .. } | Command::EditCue { sequence_id, .. } => {
                let mut images = match sequence_id {
                    Some(id) => vec![self.sequence_image(*id)],
                    // Unresolved, so this is a caller who did not go through
                    // `Self::apply` - the command is about to be refused and the
                    // record will not be filed.
                    None => Vec::new(),
                };
                images.extend(self.programmer_image());
                images
            }
            // The cue list, the programmer, **and** which cue list is in force:
            // a store onto a free number makes the list and selects it, so an
            // undo that put the show back and left the selection pointing at a
            // sequence that no longer exists would restore half a state — the
            // same fault S39's `CueEdit` image exists to prevent.
            Command::StoreSequence { sequence_id, .. } => {
                let mut images = vec![
                    self.sequence_image(*sequence_id),
                    Image::SelectedSequence(self.session.session().selected_sequence),
                ];
                images.extend(self.programmer_image());
                images
            }
            // S40's group store: the group as it stood, and the programmer,
            // because a store advances the Clear stage.
            Command::StoreGroup { group_id, .. } => {
                let mut images = vec![Image::Group(*group_id, self.show.group(*group_id).cloned())];
                images.extend(self.programmer_image());
                images
            }
            // An `Update` names its cue through the session, so its scope is
            // read from the session rather than from the command — the one
            // command whose scope is not on its face. A stale reference is not a
            // problem here: the sequence image of a sequence that has gone is
            // an absence, and the store would have been refused anyway.
            Command::Update => {
                let mut images = match &self.session.session().editing_cue {
                    Some(edit) => vec![self.sequence_image(edit.sequence_id)],
                    None => Vec::new(),
                };
                images.extend(self.programmer_image());
                images
            }
            // The preset **and** every sequence it reaches, in that order: a
            // store rewrites the cue parts linked to the preset
            // (`Show::relink`), and the sequence images are restored last so
            // that they win over what putting the old preset back relinks. See
            // [`Image::Preset`].
            Command::StorePreset { preset_id, .. } => {
                let mut images = vec![Image::Preset(
                    *preset_id,
                    self.show.preset(*preset_id).cloned(),
                )];
                images.extend(
                    self.show
                        .sequences_using_preset(*preset_id)
                        .into_iter()
                        .map(|id| self.sequence_image(id)),
                );
                images.extend(self.programmer_image());
                images
            }
            // The update state is in the scope of the two that can move it: a
            // deleted cue clears it and a renumbered cue carries it, so an Oops
            // over either has to put it back where it was (S39).
            Command::SetCueProperty { sequence_id, .. } => match sequence_id {
                Some(id) => vec![self.sequence_image(*id), self.cue_edit_image()],
                None => Vec::new(),
            },
            // **S40's four generic verbs, and their scope is their targets'.**
            // The update state travels with all four for the reason above: a
            // deleted cue clears it and a moved cue carries it, so an Oops over
            // either has to put it back where it was.
            Command::Delete { target } => {
                let mut images = self.object_image(target);
                images.push(self.cue_edit_image());
                images
            }
            // **A label through an executor images the sequence**, not the
            // executor: `Show::label_object` writes the name onto the cue list
            // on the fader, so an image of the fader is an image of the one
            // thing the command did not touch — and a record whose before and
            // after are equal is no record at all, which made `Label Executor 1`
            // silently un-undoable. Found while giving `Color` the same
            // indirection; see [`Self::colored_image`].
            Command::Label { target, .. } => {
                let mut images = self.colored_image(target);
                images.push(self.cue_edit_image());
                images
            }
            // A colour reaches one cue list and nothing else — no cue moves, so
            // the update state is not in scope the way it is for the four above.
            // **The image is the sequence's even when the line named an
            // executor**, because that is what the command writes to: an image
            // of the executor would restore a fader that never changed and
            // leave the colour where the Oops was meant to take it from.
            Command::Color { target, .. } => self.colored_image(target),
            Command::Copy { from, to, .. } | Command::Move { from, to, .. } => {
                let mut images = self.object_image(from);
                images.extend(self.object_image(to));
                // A move of a sequence repoints every executor that played it,
                // and a move of a preset rewrites every cue that linked to it -
                // so the scope is wider than the two things named. See
                // `crate::objects`.
                images.extend(self.referrer_images(from));
                images.push(self.cue_edit_image());
                images
            }
            Command::AssignExecutor { executor_id, .. } => vec![Image::Executor(
                *executor_id,
                self.show.executor(*executor_id).cloned(),
            )],
            // The three playback actions, the two journal commands, the save
            // and the fifteen session commands — named rather than caught by a
            // wildcard, so a command added to the protocol is a compile error
            // here as well as in the three appliers.
            Command::ExecutorGo { .. }
            | Command::ExecutorOff { .. }
            | Command::ExecutorOn { .. }
            | Command::Goto { .. }
            | Command::ExecutorButton { .. }
            | Command::SetExecutorMaster { .. }
            | Command::SelectSequence { .. }
            | Command::Oops
            | Command::Redo
            | Command::SaveShow
            // S37's four file commands beside it, and for the same reason
            // twice over: none of them is a show *edit*, and three of them
            // replace the show outright — after which a record filed against
            // the show that was open would describe a show that is gone.
            | Command::SaveShowAs { .. }
            | Command::OpenShow { .. }
            | Command::NewShow { .. }
            | Command::ExportShow { .. }
            | Command::ImportShow { .. }
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
            | Command::CommandLineInput { .. }
            // S33's four have no image because the journal is the **show's**:
            // it is cleared when a show is loaded and it images show and
            // session scopes, so a record of a rig change would be a record of
            // something the show it belongs to knows nothing about.
            // `Command::is_undoable` says the same thing one layer up, and
            // `a_command_has_a_scope_exactly_when_it_is_undoable` holds the two
            // together.
            | Command::AddOutput { .. }
            | Command::ConfigureOutput { .. }
            | Command::RemoveOutput { .. }
            | Command::SetOutputEnabled { .. }
            // S36's, for the same reason: a port name is not show content.
            | Command::SetSurfacePort { .. }
            | Command::ConfigureMachine { .. }
            => Vec::new(),
        }
    }

    /// One sequence as it stands, or its absence.
    fn sequence_image(&self, id: SequenceId) -> Image {
        Image::Sequence(id, self.show.sequence(id).cloned())
    }

    /// One of S40's six objects as it stands, or its absence.
    ///
    /// A **cue** images the sequence it is in, because a cue is not a thing the
    /// journal restores on its own — the cue list is. A **view** images nothing
    /// at all: it is session state, and session commands are not journalled
    /// (`ARCHITECTURE_SPEC.md` §6.1).
    fn object_image(&self, target: &ObjectRef) -> Vec<Image> {
        match target {
            ObjectRef::Sequence { sequence_id } => vec![self.sequence_image(*sequence_id)],
            ObjectRef::Cue { sequence_id, .. } => match sequence_id {
                Some(id) => vec![self.sequence_image(*id)],
                None => Vec::new(),
            },
            ObjectRef::Group { group_id } => {
                vec![Image::Group(*group_id, self.show.group(*group_id).cloned())]
            }
            ObjectRef::Preset { preset_id } => vec![Image::Preset(
                *preset_id,
                self.show.preset(*preset_id).cloned(),
            )],
            ObjectRef::Executor { executor_id } => vec![Image::Executor(
                *executor_id,
                self.show.executor(*executor_id).cloned(),
            )],
            ObjectRef::View { .. } => Vec::new(),
        }
    }

    /// The image `Command::Color` writes over: the cue list itself, or the one
    /// standing on the executor that was named.
    ///
    /// `Command::Label` has the same indirection and needs the same image, which
    /// is why this is its own function rather than an arm of
    /// [`Self::object_image`]: that one answers *what did this reference name*,
    /// and this one answers *what did the command write to*. For four of the six
    /// they are the same thing.
    fn colored_image(&self, target: &ObjectRef) -> Vec<Image> {
        match target {
            ObjectRef::Executor { executor_id } => self
                .show
                .executor(*executor_id)
                .and_then(|executor| executor.sequence_id)
                .map(|sequence_id| vec![self.sequence_image(sequence_id)])
                .unwrap_or_default(),
            other => self.object_image(other),
        }
    }

    /// The images of everything that **refers** to an object a move is about to
    /// take away — `crate::objects::repoint`'s other half.
    ///
    /// Two of them exist and both would be a half-restored Oops if they were
    /// forgotten: moving a sequence repoints every executor that played it, and
    /// moving a preset rewrites every cue part that linked to it. An undo that
    /// put the sequence back and left the fader pointing at the number it moved
    /// to would restore the show and not the desk.
    fn referrer_images(&self, from: &ObjectRef) -> Vec<Image> {
        match from {
            ObjectRef::Sequence { sequence_id } => self
                .show
                .executors()
                .filter(|executor| executor.sequence_id == Some(*sequence_id))
                .map(|executor| Image::Executor(executor.id, Some(executor.clone())))
                .collect(),
            ObjectRef::Preset { preset_id } => self
                .show
                .sequences_using_preset(*preset_id)
                .into_iter()
                .map(|id| self.sequence_image(id))
                .collect(),
            _ => Vec::new(),
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
            self.cue_edit_image(),
        ]
    }

    /// The update state as it stands, or its absence — S39.
    ///
    /// In the scope of every programmer command for the page state's reason,
    /// which [`Self::programmer_image`] gives: an undo that put the programmer
    /// back and left the desk claiming to be editing a different cue would
    /// restore half a state, and the half left standing is the one the Update
    /// key acts on.
    fn cue_edit_image(&self) -> Image {
        Image::CueEdit(self.session.session().editing_cue.clone())
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
                Image::Sequence(id, sequence) => {
                    let ops = match sequence {
                        Some(sequence) => self.show.store_sequence(sequence.clone())?,
                        None => self.show.remove_sequence(*id)?,
                    };
                    applied.deltas.push(Delta::ShowPatch { ops });
                    applied.effects.push(Effect::ReloadSequence(*id));
                }
                Image::Preset(id, preset) => {
                    let ops = match preset {
                        Some(preset) => self.show.store_preset(preset.clone())?,
                        None => self.show.remove_preset(*id)?,
                    };
                    applied.deltas.push(Delta::ShowPatch { ops });
                }
                Image::Group(id, group) => {
                    let ops = match group {
                        Some(group) => self.show.store_group(group.clone())?,
                        None => self.show.remove_group(*id)?,
                    };
                    applied.deltas.push(Delta::ShowPatch { ops });
                    applied.effects.push(Effect::ReloadGroups);
                }
                Image::Executor(id, executor) => {
                    let ops = match executor {
                        Some(executor) => self.show.store_executor(executor.clone())?,
                        None => self.show.remove_executor(*id)?,
                    };
                    applied.deltas.push(Delta::ShowPatch { ops });
                    applied.effects.push(Effect::ExecutorOff {
                        executor: prism_domain::PlaybackId::of_executor(*id),
                    });
                }
                Image::Programmer(_)
                | Image::ProgrammerPage { .. }
                | Image::CueEdit(_)
                | Image::SelectedSequence(_) => {}
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
                Image::CueEdit(editing) => {
                    let ops = self.session.set_editing_cue(editing.clone())?;
                    if !ops.is_empty() {
                        applied.deltas.push(Delta::SessionPatch { ops });
                    }
                }
                Image::SelectedSequence(sequence_id) => {
                    let ops = self.session.select_sequence(*sequence_id)?;
                    if !ops.is_empty() {
                        applied.deltas.push(Delta::SessionPatch { ops });
                    }
                }
                Image::Fixture(..)
                | Image::FixtureType(..)
                | Image::Sequence(..)
                | Image::Preset(..)
                | Image::Group(..)
                | Image::Executor(..) => {}
            }
        }
        Ok(applied)
    }
}

/// A preview of a store that would go through.
///
/// `incoming` is what the store would **write** — `Programmer::stored_keys`,
/// which is the same filter the store itself runs — and not what it would end
/// up holding. The distinction is the whole of Merge: a merged cue contains
/// everything it had before, so counting the *result* would report every value
/// the store left alone as one it had replaced.
fn counted(
    name: Option<&str>,
    stored: &[(FixtureId, AttributeType)],
    incoming: &[(FixtureId, AttributeType)],
    mode: StoreMode,
) -> StorePreview {
    let mut overlap = 0;
    let mut fresh = 0;
    for key in incoming {
        if stored.contains(key) {
            overlap += 1;
        } else {
            fresh += 1;
        }
    }
    let untouched = u32::try_from(stored.len()).unwrap_or(u32::MAX) - overlap;
    // The one place the three modes turn into numbers, and it is the table in
    // `StorePreview`'s own documentation. Writing S for what is stored and I for
    // what the programmer brings, the four counts partition S∪I every time — so
    // a mode added later that forgot a column would be visible as a sum that no
    // longer adds up rather than as a plausible wrong number.
    let (added, replaced, kept, removed) = match mode {
        StoreMode::Merge => (fresh, overlap, untouched, 0),
        StoreMode::Override => (fresh, overlap, 0, untouched),
        StoreMode::Remove => (0, 0, untouched, overlap),
    };
    StorePreview {
        accepted: true,
        refusal: None,
        exists: name.is_some(),
        name: name.unwrap_or_default().to_owned(),
        mode,
        added,
        replaced,
        kept,
        removed,
    }
}

/// The same edit, marked as having moved since the cue was loaded — S39.
///
/// One function rather than a struct-update expression in six places, so that
/// *what makes the Update key blink* has one spelling.
fn touched(edit: CueEdit) -> CueEdit {
    CueEdit {
        modified: true,
        ..edit
    }
}

/// A preview of a store that would be refused, in the refusal's own words.
fn refused(name: Option<&str>, why: &str, mode: StoreMode) -> StorePreview {
    StorePreview {
        accepted: false,
        refusal: Some(why.to_owned()),
        exists: name.is_some(),
        name: name.unwrap_or_default().to_owned(),
        mode,
        added: 0,
        replaced: 0,
        kept: 0,
        removed: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{ShowFile, ShowFileError};
    use crate::testkit::{fixture, par_type};
    use crate::{Effect, ProgrammerError, SessionError, ShowError};
    use prism_domain::{Command, Delta, StoreMode, ViewId, WindowInstanceId, WindowType};

    /// **A profile key that names nothing is refused here**, and it changes
    /// nothing — S44.
    ///
    /// The show model cannot decide this: it has no library, exactly as it has
    /// no disk. So `Show::apply` answers `Effect::EmbedProfile` and this layer,
    /// which has the library, is where the key is looked up. Asserted on the
    /// file's **serialised bytes**, which is the form S11 set for every other
    /// refusal in this crate.
    #[test]
    fn a_profile_this_desk_does_not_carry_is_refused_and_changes_nothing() {
        let mut file = file();
        let before = rmp_serde::to_vec_named(&file).unwrap();
        assert_eq!(
            file.apply(&Command::EmbedFixtureType {
                type_id: "nothing.at.all".to_owned(),
            }),
            Err(ShowFileError::Show(ShowError::UnknownLibraryType(
                "nothing.at.all".to_owned()
            )))
        );
        assert_eq!(rmp_serde::to_vec_named(&file).unwrap(), before);
        assert!(!file.is_dirty(), "a refusal does not light the Save lamp");
        assert!(file.journal.is_empty(), "nor does it file a step");

        // And the one it does carry is embedded, with the deltas that describe
        // it and the effect the engine needs.
        let applied = file
            .apply(&Command::EmbedFixtureType {
                type_id: "generic.rgb.par".to_owned(),
            })
            .expect("the desk carries an RGB PAR");
        assert!(applied.effects.contains(&Effect::Repatch));
        assert!(
            applied
                .deltas
                .iter()
                .any(|delta| matches!(delta, Delta::ShowPatch { .. }))
        );
        assert!(file.show.fixture_type("generic.rgb.par").is_some());
        // The library is the *desk's*, and embedding took a copy: the show now
        // owns it, which is what makes a show open on a desk without it.
        assert_eq!(
            file.show.fixture_type("generic.rgb.par"),
            file.library.profile("generic.rgb.par")
        );
    }

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
                sequence_id: Some(prism_domain::SequenceId::new(1)),
                cue_number: "1".to_owned(),
                mode: StoreMode::Merge,
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
            AttributeType, ExecutorId, FeatureGroup, FixtureId, GoDirection, ObjectRef,
            OverwriteMode, ParamDirection, PlaybackTarget, PresetId, SelectionMode, SequenceId,
            SequenceStoreMode, StoreMode,
        };

        let file = file();
        // A sample of `docs/IPC_PROTOCOL.md` §5 rather than all forty of it —
        // `tests/command_application.rs` is what holds the whole list — and it
        // has to contain **every command with a scope**, or a new one could be
        // given an image here and left out of `is_undoable` without either
        // saying so. The undoable count below is the guard on that.
        let commands = [
            Command::SelectFixtures {
                ids: vec![FixtureId::new(1)],
                mode: SelectionMode::Set,
            },
            Command::SelectGroup {
                group_id: prism_domain::GroupId::new(1),
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
                sequence_id: Some(SequenceId::new(1)),
                cue_number: "1".to_owned(),
                mode: StoreMode::Merge,
            },
            Command::ExecutorGo {
                target: PlaybackTarget::of_executor(ExecutorId::new(0)),
                direction: GoDirection::Next,
            },
            Command::ExecutorOff {
                target: PlaybackTarget::of_executor(ExecutorId::new(0)),
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
            // The four S28 added that write show content, and S39's four.
            Command::StorePreset {
                preset_id: PresetId::new(1),
                pool: Some(FeatureGroup::Color),
                name: String::new(),
                color: None,
                mode: StoreMode::Merge,
            },
            Command::StoreSequence {
                sequence_id: SequenceId::new(9),
                name: String::new(),
                mode: SequenceStoreMode::Append,
            },
            Command::SetCueProperty {
                sequence_id: Some(SequenceId::new(1)),
                cue_number: "1".to_owned(),
                property: prism_domain::CueProperty::FadeIn { seconds: 0.0 },
            },
            // S40's four generic verbs, each of which is a show edit here and
            // would be a session edit with a view as its target.
            Command::Label {
                target: ObjectRef::Cue {
                    sequence_id: Some(SequenceId::new(1)),
                    cue_number: "1".to_owned(),
                },
                name: String::new(),
            },
            Command::Copy {
                from: ObjectRef::Group {
                    group_id: prism_domain::GroupId::new(1),
                },
                to: ObjectRef::Group {
                    group_id: prism_domain::GroupId::new(2),
                },
                mode: OverwriteMode::Merge,
            },
            Command::Move {
                from: ObjectRef::Preset {
                    preset_id: PresetId::new(1),
                },
                to: ObjectRef::Preset {
                    preset_id: PresetId::new(2),
                },
                mode: OverwriteMode::Merge,
            },
            Command::StoreGroup {
                group_id: prism_domain::GroupId::new(1),
                name: String::new(),
                mode: OverwriteMode::Merge,
            },
            Command::Delete {
                target: ObjectRef::Cue {
                    sequence_id: Some(SequenceId::new(1)),
                    cue_number: "1".to_owned(),
                },
            },
            Command::AssignExecutor {
                executor_id: ExecutorId::new(0),
                sequence_id: None,
            },
            Command::StoreSequence {
                sequence_id: SequenceId::new(1),
                name: String::new(),
                mode: SequenceStoreMode::Append,
            },
            Command::EditCue {
                sequence_id: Some(SequenceId::new(1)),
                cue_number: "1".to_owned(),
            },
            Command::Update,
            // A session command, so it has no scope — the one of S39's four
            // that an Oops must not reach.
            Command::SelectSequence {
                sequence_id: SequenceId::new(1),
            },
        ];
        assert_eq!(commands.len(), 38);
        let mut undoable = 0;
        for command in &commands {
            assert_eq!(
                !file.image(command).is_empty(),
                command.is_undoable(),
                "{command:?}"
            );
            undoable += usize::from(command.is_undoable());
        }
        // What is left after the playback actions, `Oops`, `Redo`, `SaveShow`
        // and the §4.4 session commands: the five programmer commands, the
        // patch, S28's show edits, S39's three and S40's five — the four
        // generic verbs and the group store.
        assert_eq!(undoable, 19);
    }
}
