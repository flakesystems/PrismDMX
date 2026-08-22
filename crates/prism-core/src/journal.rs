//! The Oops journal — `ARCHITECTURE_SPEC.md` §6.1.
//!
//! > Applying a command produces a compact `UndoRecord` holding the inverse and
//! > the affected scope, kept in a 200-entry ring buffer.
//!
//! # Why the scope is the whole design, and not bookkeeping
//!
//! The obvious journal is a stack of two hundred copies of the show. It is
//! wrong twice. It is expensive — a show is the patch, the embedded profiles
//! and every stored look — and, far worse, it takes back things the operator
//! never asked to take back. `SetExecutorMaster` **is** show state: an executor
//! master lives in the show and is written by a command. It is also, by
//! `Command::is_undoable`, deliberately *not* undoable, because §6.1 forbids an
//! undo from changing light the operator is currently driving. A snapshot
//! journal cannot honour both facts at once: undoing a patch would pull the
//! fader the operator moved after it back down with it.
//!
//! So a record names what its command touched and carries only that:
//!
//! | Command | Scope |
//! |---|---|
//! | `PatchFixture`, `UnpatchFixture` | that one fixture's patch entry |
//! | `RenumberFixture` | **both** numbers' patch entries — the one it left and the one it took |
//! | `EmbedFixtureType` | that one embedded profile |
//! | `StoreCue`, `StoreSequence`, `EditCue`, `Update` | that one sequence and the whole desk state a programmer command moves |
//! | `SelectFixtures`, `SetAttribute`, `ApplyPreset`, `ClearProgrammer` | the desk state, which is the programmer, its page state and the update state |
//! | `CreateSequence`, `SetCueProperty`, `DeleteCue` | that one sequence, and the update state it can clear |
//! | `StorePreset` | that preset, every sequence it reaches, and the desk state |
//! | `AssignExecutor` | that one executor slot |
//!
//! Those are exactly the undoable commands: the forty of
//! `docs/IPC_PROTOCOL.md` §5 less the four playback actions, less `Oops`,
//! `Redo` and `SaveShow`, less the sixteen §4.4 session commands.
//!
//! # Why the record holds two images rather than one inverse
//!
//! Undo needs the state before the command; redo needs the state after it. Both
//! are the same shape, both are already to hand at the moment the command is
//! applied, and computing the second from the first would mean inverting an
//! inverse — so a record is a before-image and an after-image of the same
//! scope. Restoring one *is* applying the inverse.
//!
//! # The programmer is journaled whole, and the show is not
//!
//! `docs/DMX_MERGE.md` §3 makes the programmer sparse by specification, so its
//! whole state is a selection, a handful of touched values and a button stage —
//! small enough that describing a change to it would cost more than copying it.
//! S13 built [`Programmer::restore`](crate::Programmer::restore) for exactly
//! this. A show is the opposite, which is why a show image is one fixture or one
//! sequence.
//!
//! # Not persisted
//!
//! A record describes a step between two states of *this* show in *this* run of
//! the daemon. Restored from disk beside a file that may have been edited by
//! hand since, its inverse would be an assertion about a show that no longer
//! exists — so [`ShowFile`](crate::ShowFile) keeps the journal `#[serde(skip)]`
//! and a reopened show has nothing to undo. **S15 requirement:** a loader that
//! replaces the models in place calls [`Journal::clear`]; one that deserialises
//! a fresh file gets an empty journal for free.

use core::fmt;
use std::collections::VecDeque;

use prism_domain::{
    Command, CueEdit, Executor, ExecutorId, Fixture, FixtureId, FixtureType, Group, GroupId,
    Preset, PresetId, ProgrammerState, Sequence, SequenceId,
};

/// Why an Oops or a Redo could not be carried out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalError {
    /// Nothing has been done that this journal can take back.
    NothingToUndo,
    /// Nothing has been taken back that this journal can put back.
    NothingToRedo,
}

impl fmt::Display for JournalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NothingToUndo => write!(f, "there is nothing to undo"),
            Self::NothingToRedo => write!(f, "there is nothing to redo"),
        }
    }
}

impl core::error::Error for JournalError {}

/// The part of the state one record covers — §6.1's "affected scope".
///
/// What a client shows beside an Oops button, and what makes the exclusion of
/// the playback commands real rather than promised: a record that names one
/// fixture cannot move an executor.
/// `Copy` was dropped in S27, when `EmbedFixtureType` gave one variant a key
/// rather than a number. A scope is compared and printed, never counted on in a
/// hot path, so a move is not a cost worth naming a variant vaguely to avoid.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UndoScope {
    /// One fixture's entry in the patch.
    Fixture(FixtureId),
    /// One embedded profile.
    FixtureType(String),
    /// One sequence, with its cues.
    Sequence(SequenceId),
    /// One preset.
    Preset(PresetId),
    /// One group (S40).
    Group(GroupId),
    /// One executor slot.
    Executor(ExecutorId),
    /// The programmer, whole.
    Programmer,
    /// The session's programmer page and jog-wheel parameter index — the two
    /// §4.1 fields a programmer command reaches into (S13).
    ProgrammerPage,
    /// The session's update state: which cue the programmer is editing, and
    /// whether it has moved since (S39).
    CueEdit,
    /// Which cue list a bare `Store Cue 5` goes into — `Session::selected_sequence`
    /// (S40).
    SelectedSequence,
}

/// One piece of state, as it stood at one moment.
///
/// Private to the crate: the images are what the journal restores, and a caller
/// able to build one could write any state it liked into the show without going
/// through the validation every edit otherwise passes. [`UndoRecord::scope`] is
/// the public half.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Image {
    /// The patch entry for a fixture. `None`: it was not patched.
    ///
    /// The absence is a real case and not defensive: patching a fixture for the
    /// first time has "not patched" as its inverse.
    Fixture(FixtureId, Option<Fixture>),
    /// One embedded profile. `None`: the show did not carry it.
    ///
    /// Embedding one for the first time has "not embedded" as its inverse, the
    /// same shape a fixture has — and for the same reason, since S27 made both
    /// reachable from the interface.
    FixtureType(String, Option<FixtureType>),
    /// A sequence and its cues. `None`: the show did not have it.
    ///
    /// The absence arrived in **S28**, exactly where the note here said it
    /// would: `Command::CreateSequence` makes a sequence that was not there, so
    /// "no sequence" is now the inverse of an ordinary edit rather than a state
    /// no record could be in.
    Sequence(SequenceId, Option<Sequence>),
    /// One preset. `None`: the show did not carry it.
    ///
    /// A `StorePreset` images the **sequences that reference the preset** as
    /// well, because storing a preset rewrites the cue parts linked to it
    /// (`Show::relink`). Restoring the preset alone would take the edit back in
    /// the pool and leave it standing in every cue — and where the preset was
    /// created by the store, restoring it means removing it, which relinks
    /// nothing at all.
    Preset(PresetId, Option<Preset>),
    /// One group. `None`: the show did not have it.
    ///
    /// New in **S40**, which gave `Show::store_group` the command it had been
    /// waiting for since S11 and gave the pool `Delete`, `Copy`, `Move` and
    /// `Label` beside it.
    Group(GroupId, Option<Group>),
    /// One executor slot. `None`: the slot was empty.
    Executor(ExecutorId, Option<Executor>),
    /// The whole programmer state, Clear stage included.
    Programmer(ProgrammerState),
    /// The session's page state.
    ProgrammerPage {
        /// `Session::programmer_page`.
        page: u32,
        /// `Session::programmer_param_index`.
        param_index: u32,
    },
    /// The session's update state — `Session::editing_cue` (S39). `None`: the
    /// programmer was editing nothing.
    ///
    /// **In the scope for [`Self::ProgrammerPage`]'s reason**, which §6.1's
    /// exclusion of the session *commands* does not contradict: the exclusion is
    /// about an undo pulling windows out from under an operator, and this is the
    /// cursor into the cue the programmer came from. An Oops over an `EditCue`
    /// that put the programmer back and left the desk claiming to be editing cue
    /// 3 would restore half a state — and the half it left standing is the one
    /// an Update key acts on.
    CueEdit(Option<CueEdit>),
    /// The cue list a bare cue number means — `Session::selected_sequence`.
    /// `None`: none was selected.
    ///
    /// New in **S40**, and for one command only: `Store Sequence 4` on a free
    /// number *makes* the cue list and puts it in force, so an Oops that took
    /// the cue list back and left the desk pointing at it would leave the next
    /// `Store Cue 1` naming a sequence that is not there. `SelectSequence`
    /// itself is a session command and is not journalled — the exclusion §4.1
    /// makes, for the reason a view switch is not undoable — so this image
    /// exists to keep the *show* command whole rather than to make selecting
    /// undoable.
    SelectedSequence(Option<SequenceId>),
}

impl Image {
    /// What this image is an image of.
    pub(crate) fn scope(&self) -> UndoScope {
        match self {
            Self::Fixture(id, _) => UndoScope::Fixture(*id),
            Self::FixtureType(type_id, _) => UndoScope::FixtureType(type_id.clone()),
            Self::Sequence(id, _) => UndoScope::Sequence(*id),
            Self::Preset(id, _) => UndoScope::Preset(*id),
            Self::Group(id, _) => UndoScope::Group(*id),
            Self::Executor(id, _) => UndoScope::Executor(*id),
            Self::Programmer(_) => UndoScope::Programmer,
            Self::ProgrammerPage { .. } => UndoScope::ProgrammerPage,
            Self::CueEdit(_) => UndoScope::CueEdit,
            Self::SelectedSequence(_) => UndoScope::SelectedSequence,
        }
    }
}

/// One step, and how to take it back or take it again.
#[derive(Debug, Clone, PartialEq)]
pub struct UndoRecord {
    command: Command,
    before: Vec<Image>,
    after: Vec<Image>,
}

impl UndoRecord {
    /// A record over one scope, imaged before and after.
    pub(crate) const fn new(command: Command, before: Vec<Image>, after: Vec<Image>) -> Self {
        Self {
            command,
            before,
            after,
        }
    }

    /// The command that produced this step.
    ///
    /// **S26 requirement:** this is what names the Oops on the console — "Oops:
    /// Store Cue" tells an operator what is about to happen, where "Oops" alone
    /// asks them to guess.
    #[must_use]
    pub const fn command(&self) -> &Command {
        &self.command
    }

    /// What this record covers, in the order it is restored.
    #[must_use]
    pub fn scope(&self) -> Vec<UndoScope> {
        self.before.iter().map(Image::scope).collect()
    }

    /// The state as it was before the command.
    pub(crate) fn before(&self) -> &[Image] {
        &self.before
    }

    /// The state as it was after the command.
    pub(crate) fn after(&self) -> &[Image] {
        &self.after
    }

    /// Whether the command changed anything at all in its own scope.
    ///
    /// A command that was accepted and moved nothing is not a step: an operator
    /// pressing Oops after one would watch nothing happen and press it again,
    /// losing the edit they actually meant to take back.
    pub(crate) fn is_a_step(&self) -> bool {
        self.before != self.after
    }
}

/// The 200-entry ring of `ARCHITECTURE_SPEC.md` §6.1, and the redo stack beside
/// it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Journal {
    /// Steps taken, oldest first. Bounded by [`Journal::CAPACITY`].
    done: VecDeque<UndoRecord>,
    /// Steps taken back, most recently undone last. Bounded by `done`, which is
    /// the only thing that fills it.
    undone: Vec<UndoRecord>,
}

impl Journal {
    /// How many steps can be taken back — §6.1's ring.
    ///
    /// Two hundred edits is far more than a session of programming between
    /// saves, and the bound exists so that a desk left running for a week
    /// cannot grow a journal until it runs out of memory.
    pub const CAPACITY: usize = 200;

    /// An empty journal.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many steps can be taken back.
    #[must_use]
    pub fn len(&self) -> usize {
        self.done.len()
    }

    /// Whether there is anything to take back.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.done.is_empty()
    }

    /// How many steps can be put back.
    #[must_use]
    pub fn redo_len(&self) -> usize {
        self.undone.len()
    }

    /// The step the next Oops would take back.
    #[must_use]
    pub fn undoable(&self) -> Option<&UndoRecord> {
        self.done.back()
    }

    /// The step the next Redo would put back.
    #[must_use]
    pub fn redoable(&self) -> Option<&UndoRecord> {
        self.undone.last()
    }

    /// Forgets everything. **S15 requirement:** loading a show calls this.
    pub fn clear(&mut self) {
        self.done.clear();
        self.undone.clear();
    }

    /// Files a step, dropping the oldest if the ring is full.
    ///
    /// A new step also forgets the redo stack: the operator has taken a
    /// different branch, and putting back a command from the branch they left
    /// would interleave two histories into one that never happened.
    pub(crate) fn push(&mut self, record: UndoRecord) {
        self.undone.clear();
        if self.done.len() == Self::CAPACITY {
            self.done.pop_front();
        }
        self.done.push_back(record);
    }

    /// Takes the newest step off the undo stack.
    pub(crate) fn take_undo(&mut self) -> Option<UndoRecord> {
        self.done.pop_back()
    }

    /// Takes the newest step off the redo stack.
    pub(crate) fn take_redo(&mut self) -> Option<UndoRecord> {
        self.undone.pop()
    }

    /// Puts a step back where an undo found it — a restore that was refused, or
    /// one that has just been redone.
    pub(crate) fn put_undo(&mut self, record: UndoRecord) {
        self.done.push_back(record);
    }

    /// Puts a step on the redo stack — one that has just been undone, or a redo
    /// that was refused.
    pub(crate) fn put_redo(&mut self, record: UndoRecord) {
        self.undone.push(record);
    }
}

#[cfg(test)]
mod tests {
    use super::{Image, Journal, JournalError, UndoRecord, UndoScope};
    use prism_domain::{
        Command, FixtureId, ProgrammerState, Sequence, SequenceId, SequenceStoreMode,
    };

    fn record(id: u32) -> UndoRecord {
        UndoRecord::new(
            Command::ClearProgrammer,
            vec![Image::Fixture(FixtureId::new(id), None)],
            vec![Image::Programmer(ProgrammerState::default())],
        )
    }

    #[test]
    fn the_ring_holds_two_hundred_and_drops_the_oldest() {
        let mut journal = Journal::new();
        assert!(journal.is_empty());
        for id in 0..u32::try_from(Journal::CAPACITY).unwrap() + 50 {
            journal.push(record(id));
        }
        assert_eq!(journal.len(), Journal::CAPACITY);
        assert!(!journal.is_empty());
        // The newest is the last one pushed and the oldest is the fifty-first,
        // so the ring dropped from the front rather than refusing at the back.
        assert_eq!(
            journal.undoable().map(UndoRecord::scope),
            Some(vec![UndoScope::Fixture(FixtureId::new(249))])
        );
        let mut taken = Vec::new();
        while let Some(entry) = journal.take_undo() {
            taken.push(entry.scope());
        }
        assert_eq!(taken.len(), Journal::CAPACITY);
        assert_eq!(
            taken.last(),
            Some(&vec![UndoScope::Fixture(FixtureId::new(50))])
        );
    }

    #[test]
    fn a_new_step_forgets_the_redo_stack() {
        let mut journal = Journal::new();
        journal.push(record(1));
        let undone = journal.take_undo().unwrap();
        journal.put_redo(undone);
        assert_eq!(journal.redo_len(), 1);
        assert!(journal.redoable().is_some());

        journal.push(record(2));
        assert_eq!(journal.redo_len(), 0);
        assert!(journal.redoable().is_none());
        assert!(journal.take_redo().is_none());
    }

    #[test]
    fn a_step_that_was_refused_goes_back_where_it_came_from() {
        let mut journal = Journal::new();
        journal.push(record(1));
        let taken = journal.take_undo().unwrap();
        assert_eq!(journal.len(), 0);
        journal.put_undo(taken);
        assert_eq!(journal.len(), 1);

        journal.clear();
        assert!(journal.is_empty());
        assert_eq!(journal.redo_len(), 0);
    }

    #[test]
    fn a_record_names_its_command_its_scope_and_whether_it_moved_anything() {
        let entry = record(1);
        assert_eq!(entry.command(), &Command::ClearProgrammer);
        assert_eq!(entry.scope(), vec![UndoScope::Fixture(FixtureId::new(1))]);
        assert!(entry.is_a_step());
        assert_eq!(
            entry.before()[0].scope(),
            UndoScope::Fixture(FixtureId::new(1))
        );
        assert_eq!(entry.after()[0].scope(), UndoScope::Programmer);

        let unchanged = UndoRecord::new(
            Command::ClearProgrammer,
            vec![Image::ProgrammerPage {
                page: 0,
                param_index: 0,
            }],
            vec![Image::ProgrammerPage {
                page: 0,
                param_index: 0,
            }],
        );
        assert!(!unchanged.is_a_step());
        assert_eq!(unchanged.scope(), vec![UndoScope::ProgrammerPage]);
    }

    #[test]
    fn a_sequence_image_names_its_sequence() {
        let image = Image::Sequence(
            SequenceId::new(3),
            Some(Sequence {
                id: SequenceId::new(3),
                name: "Sequence 3".to_owned(),
                color: None,
                cues: Vec::new(),
                looping: false,
                is_active: false,
                current_cue_index: None,
            }),
        );
        assert_eq!(image.scope(), UndoScope::Sequence(SequenceId::new(3)));
    }

    /// **An absent sequence is still a scope**, which is what S28's
    /// `CreateSequence` needed: the inverse of creating one is *there is no
    /// sequence 3*, and a record has to be able to say that.
    #[test]
    fn a_sequence_that_is_not_there_names_its_number_all_the_same() {
        let image = Image::Sequence(SequenceId::new(3), None);
        assert_eq!(image.scope(), UndoScope::Sequence(SequenceId::new(3)));

        let created = UndoRecord::new(
            Command::StoreSequence {
                sequence_id: SequenceId::new(3),
                name: "Act 1".to_owned(),
                mode: SequenceStoreMode::Append,
            },
            vec![image],
            vec![Image::Sequence(
                SequenceId::new(3),
                Some(Sequence {
                    id: SequenceId::new(3),
                    name: "Act 1".to_owned(),
                    color: None,
                    cues: Vec::new(),
                    looping: false,
                    is_active: false,
                    current_cue_index: None,
                }),
            )],
        );
        assert!(created.is_a_step());
        assert_eq!(
            created.scope(),
            vec![UndoScope::Sequence(SequenceId::new(3))]
        );
    }

    /// **The update state is a scope of its own**, present or not — S39. An
    /// Oops over an `EditCue` has to be able to say *the programmer was editing
    /// nothing*, which is the same absence a created sequence needed in S28.
    #[test]
    fn a_cue_edit_image_names_itself_whether_or_not_a_cue_was_loaded() {
        assert_eq!(Image::CueEdit(None).scope(), UndoScope::CueEdit);
        let loaded = Image::CueEdit(Some(prism_domain::CueEdit {
            sequence_id: SequenceId::new(1),
            cue_number: "3".to_owned(),
            modified: false,
        }));
        assert_eq!(loaded.scope(), UndoScope::CueEdit);
        assert!(
            UndoRecord::new(
                Command::EditCue {
                    sequence_id: Some(SequenceId::new(1)),
                    cue_number: "3".to_owned(),
                },
                vec![Image::CueEdit(None)],
                vec![loaded],
            )
            .is_a_step()
        );
    }

    /// A preset and an executor each name themselves, present or not.
    #[test]
    fn a_preset_and_an_executor_image_name_what_they_are_of() {
        assert_eq!(
            Image::Preset(prism_domain::PresetId::new(4), None).scope(),
            UndoScope::Preset(prism_domain::PresetId::new(4))
        );
        assert_eq!(
            Image::Executor(prism_domain::ExecutorId::new(2), None).scope(),
            UndoScope::Executor(prism_domain::ExecutorId::new(2))
        );
    }

    #[test]
    fn the_two_refusals_read_as_themselves() {
        assert_eq!(
            JournalError::NothingToUndo.to_string(),
            "there is nothing to undo"
        );
        assert_eq!(
            JournalError::NothingToRedo.to_string(),
            "there is nothing to redo"
        );
    }
}
