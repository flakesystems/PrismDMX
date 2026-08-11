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
//! copy it — see that module for why. **S15 requirement:** this type is what a
//! `.prism` file holds, and it has no field for a desk.

use prism_domain::{ClearStage, Command, Delta, NoticeLevel};
use serde::{Deserialize, Serialize};

use crate::command::{Applied, Effect};
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
}

impl core::fmt::Display for ShowFileError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Show(error) => error.fmt(f),
            Self::Session(error) => error.fmt(f),
            Self::Programmer(error) => error.fmt(f),
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
    /// `Delta::DirtyFlag` when there is news. **S15 requirement:** called when
    /// the write has succeeded, not when it starts.
    pub const fn mark_saved(&mut self) -> bool {
        // Both, always: a save writes both halves, so leaving either flag
        // standing would light the lamp over a file that is on disk.
        let show = self.show.mark_saved();
        let session = self.session.mark_saved();
        show || session
    }

    /// Routes a command to the applier that owns it and applies it.
    ///
    /// # Errors
    ///
    /// [`ShowFileError`] if the command was refused. No half is changed after
    /// an error.
    pub fn apply(&mut self, command: &Command) -> Result<Applied, ShowFileError> {
        let was_dirty = self.is_dirty();
        let mut applied = if command.is_session_command() {
            self.session.apply(command)?
        } else {
            self.show.apply(command)?
        };
        if applied.effects.contains(&Effect::Programmer) {
            // The show has decided its half and named who finishes the job.
            // That somebody is here, so the effect is carried out rather than
            // handed on.
            applied
                .effects
                .retain(|effect| *effect != Effect::Programmer);
            let finished = self.finish_programmer(command)?;
            applied.deltas.extend(finished.deltas);
            applied.effects.extend(finished.effects);
        }
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
    }
}
