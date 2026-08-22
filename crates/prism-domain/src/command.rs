//! Commands — `docs/IPC_PROTOCOL.md` §5.
//!
//! A command expresses intent. The daemon validates it, applies it, journals it
//! for Oops where applicable and broadcasts the resulting [`crate::Delta`]. A
//! command that cannot be applied changes nothing.
//!
//! The second half of the list is the concrete form of decision **D11**: the
//! console and the UI draw on one vocabulary, so there is no second command
//! world to keep in sync.

use core::fmt;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    AttributeType, CueProperty, ExecutorButtonRef, ExecutorId, FeatureGroup, FixtureId, GroupId,
    JsonValue, OutputId, OutputInstance, OutputKind, PlaybackTarget, PresetId, RgbColor,
    SequenceId, StoreMode, UniverseId, ViewId, WindowInstanceId, WindowType,
};

/// How a selection command combines with the existing selection.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum SelectionMode {
    /// Replace the selection.
    #[default]
    Set,
    /// Add to the selection.
    Add,
    /// Toggle each fixture's membership.
    Toggle,
}

/// Which way an executor steps.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum GoDirection {
    /// To the next cue.
    #[default]
    Next,
    /// Back to the previous cue.
    Prev,
}

/// How a store into a whole **sequence** combines with the cue list that is
/// there — `Command::StoreSequence` (S39).
///
/// A deliberately different three from [`crate::StoreMode`], because a sequence
/// store is about *cues* where a cue store is about *values*. What each name
/// means one level up is the same thing it means one level down: append leaves
/// everything and adds, override replaces the lot, merge writes into what is
/// already there.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum SequenceStoreMode {
    /// A **new cue at the highest number**, holding what the programmer holds.
    ///
    /// The number is one past the highest whole number in the list, so a list of
    /// `1`, `1.5`, `2` gains a `3` — an operator building a show presses this
    /// over and over and gets `1`, `2`, `3`, which is what a cue list looks
    /// like. The default, because it is the one that cannot lose a cue.
    #[default]
    Append,
    /// The sequence **becomes** this look: one cue, numbered `1`, and the cue
    /// list that was there is gone.
    ///
    /// The most destructive command in `docs/IPC_PROTOCOL.md` §5, and the reason
    /// it is a mode on a command rather than a command of its own is that an
    /// operator chooses between the three in one gesture. It is undoable like
    /// every other show edit (`ARCHITECTURE_SPEC.md` §6.1), and an interface
    /// offering it should say what it will cost first.
    Override,
    /// The look is merged into **every cue** of the sequence.
    ///
    /// *Add this to the whole list* — the cue-level [`crate::StoreMode::Merge`]
    /// applied to each cue in turn, so a colour added to a ten-cue list is one
    /// gesture rather than ten. Refused on a sequence with no cues: there is
    /// nothing to merge into, and silently appending instead would be a
    /// different command than the one that was sent.
    Merge,
}

/// Which way the programmer parameter selection moves.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum ParamDirection {
    /// To the previous parameter.
    #[default]
    Prev,
    /// To the next parameter.
    Next,
}

/// One thing about a configured output that `Command::ConfigureOutput` changes
/// *(S33)*.
///
/// **One field per command, for `CueProperty`'s reason.** The alternative — one
/// command carrying the whole [`OutputInstance`] — makes a client read the row,
/// change one member and send the rest back, which is a read-modify-write over
/// state the daemon owns: two people in a settings window, one re-addressing a
/// node and one renaming it, would each undo the other and neither would have
/// done anything wrong.
///
/// The `id` and the `enabled` flag are not here. The number is the key the
/// output is filed under — changing it is `RemoveOutput` and `AddOutput`, said
/// out loud — and enabling is [`Command::SetOutputEnabled`], which is a switch
/// an operator flips at a rack rather than a field they edit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(tag = "t", rename_all_fields = "camelCase")]
pub enum OutputChange {
    /// What the operator calls it. Costs the rig nothing — see
    /// [`OutputInstance::needs_restart`].
    Name {
        /// The new name.
        name: String,
    },
    /// The interface and its parameters: a node's address, an adapter's serial,
    /// the port mapping, the hop limit.
    Kind {
        /// The new kind.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        kind: OutputKind,
    },
    /// Which of the desk's universes this interface carries.
    Universes {
        /// The new set, in send order.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::small_vec(4)")
        )]
        universes: Vec<UniverseId>,
    },
}

/// One of the numbered things on a desk — what `Delete`, `Copy`, `Move` and
/// `Label` name (S40).
///
/// # Why one type rather than four commands each
///
/// S40 made the command line the interface (`ARCHITECTURE_SPEC.md` §4.5), and
/// the grammar it needs is one production:
///
/// ```text
///   verb := "delete" | "copy" | "move" | "label"
///   line := verb object number [object number] [name]
/// ```
///
/// *Delete sequence 4* and *delete group 4* are the same act on two things, and
/// an operator learns one word rather than six. Writing them as six commands
/// apiece would have put twenty-four variants into `docs/IPC_PROTOCOL.md` §5
/// whose only difference is which pool they index, and — worse — it would have
/// made the parser choose the command, which is the client deciding what a line
/// *means* rather than what it *says*.
///
/// The price is that a command carrying one of these is show state for five of
/// the six and **session** state for [`Self::View`], since
/// `ARCHITECTURE_SPEC.md` §4.1 puts the view library in the session. So
/// [`Command::is_session_command`] reads the target instead of matching on the
/// variant alone. That is a real cost and it is paid once, here, rather than by
/// an operator learning that views are renamed with a different word.
///
/// # A cue names its sequence, or does not
///
/// `Delete Cue 3` typed on the command line names no cue list, and it means the
/// one the session has selected — `Session::selectedSequence`, which is S39's
/// field and the reason it exists (§4.1). The daemon resolves it, and a client
/// that filled the number in for itself would be sending a command whose
/// meaning had already moved on a second screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(tag = "t", rename_all_fields = "camelCase")]
pub enum ObjectRef {
    /// A cue list.
    Sequence {
        /// The sequence number.
        sequence_id: SequenceId,
    },
    /// One cue of a cue list.
    Cue {
        /// The sequence, or `None` for the one the session has selected.
        #[serde(default)]
        sequence_id: Option<SequenceId>,
        /// The cue, by the number an operator typed.
        cue_number: String,
    },
    /// A fixture group.
    Group {
        /// The group number.
        group_id: GroupId,
    },
    /// A preset, in whichever pool it is filed in.
    ///
    /// Numbers are unique **across** pools (`prism_core::Show::store_preset`),
    /// so a preset is reached by number alone.
    Preset {
        /// The preset number.
        preset_id: PresetId,
    },
    /// A stored canvas layout. **Session state** — see the type documentation.
    View {
        /// The view number.
        view_id: ViewId,
    },
    /// One slot of the executor grid, `page * 8 + slot` (**D7**).
    Executor {
        /// The executor number.
        executor_id: ExecutorId,
    },
}

impl ObjectRef {
    /// Whether this names something in the **session** rather than in the show.
    ///
    /// One variant does, and [`Command::is_session_command`] is why it matters:
    /// `ARCHITECTURE_SPEC.md` §4.1 puts the view library in the session, so a
    /// `Delete` of a view is applied by the session applier and journalled the
    /// way every session command is — that is, not at all (§6.1).
    #[must_use]
    pub const fn is_session_object(&self) -> bool {
        matches!(self, Self::View { .. })
    }

    /// The word an operator would use for this kind of thing.
    ///
    /// Used in refusals, so it is the word the command line takes rather than a
    /// Rust identifier.
    #[must_use]
    pub const fn noun(&self) -> &'static str {
        match self {
            Self::Sequence { .. } => "sequence",
            Self::Cue { .. } => "cue",
            Self::Group { .. } => "group",
            Self::Preset { .. } => "preset",
            Self::View { .. } => "view",
            Self::Executor { .. } => "executor",
        }
    }

    /// Whether two references name the same **kind** of thing.
    ///
    /// `Copy Cue 2 Group 6` is a line the parser will happily build and nothing
    /// can carry out, so it is refused rather than interpreted — the same split
    /// `Command::SelectFixtures` has with a fixture the rig has not got.
    #[must_use]
    pub const fn same_kind_as(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Sequence { .. }, Self::Sequence { .. })
                | (Self::Cue { .. }, Self::Cue { .. })
                | (Self::Group { .. }, Self::Group { .. })
                | (Self::Preset { .. }, Self::Preset { .. })
                | (Self::View { .. }, Self::View { .. })
                | (Self::Executor { .. }, Self::Executor { .. })
        )
    }
}

impl fmt::Display for ObjectRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sequence { sequence_id } => write!(f, "sequence {sequence_id}"),
            Self::Cue {
                sequence_id: Some(sequence),
                cue_number,
            } => write!(f, "sequence {sequence} cue {cue_number}"),
            Self::Cue {
                sequence_id: None,
                cue_number,
            } => write!(f, "cue {cue_number}"),
            Self::Group { group_id } => write!(f, "group {group_id}"),
            Self::Preset { preset_id } => write!(f, "preset {preset_id}"),
            Self::View { view_id } => write!(f, "view {view_id}"),
            Self::Executor { executor_id } => write!(f, "executor {executor_id}"),
        }
    }
}

/// What a `Copy`, a `Move` or a `Store Group` does when the destination is
/// already taken (S40).
///
/// Two values and not [`crate::StoreMode`]'s three, because *remove* means
/// nothing here: a copy takes what is in one place and puts it in another, and
/// there is no third thing it could do. The command line offers **merge,
/// override or cancel**, and cancel is not a mode — it is the operator not
/// sending the command, which is why it is not a variant.
///
/// `Merge` is the default for [`crate::StoreMode::Merge`]'s reason: it is the
/// one that cannot lose anything.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum OverwriteMode {
    /// Write into what is there and leave the rest standing.
    ///
    /// A cue keeps the parts the source does not mention, a group keeps the
    /// fixtures, a view keeps the windows, a sequence keeps the cue numbers the
    /// source has none of.
    #[default]
    Merge,
    /// Make the destination **exactly** what the source is.
    ///
    /// What it does not mention is gone. This is the destructive one, and it is
    /// why the interface asks before sending it.
    Override,
}

/// Everything a client — or the surface controller — can ask the daemon to do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(tag = "t", rename_all_fields = "camelCase")]
pub enum Command {
    /// Change the programmer selection.
    SelectFixtures {
        /// The fixtures to select.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::small_vec(4)")
        )]
        ids: Vec<FixtureId>,
        /// How to combine with the current selection.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        mode: SelectionMode,
    },
    /// Set an attribute on the current selection.
    SetAttribute {
        /// The attribute to change.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        attribute: AttributeType,
        /// Absolute value `0..=65535`, or a signed delta when `relative` is set.
        ///
        /// Signed because encoders turn both ways: `ARCHITECTURE_SPEC.md` §6
        /// writes this as a plain `number`, which cannot be a `u16` and still
        /// express a decrement.
        value: i32,
        /// Whether `value` is a delta rather than an absolute value.
        relative: bool,
    },
    /// Select every fixture of a group — S40's `Group 3`.
    ///
    /// **Not [`Self::SelectFixtures`] with the members filled in**, and the
    /// difference is D3: which fixtures a group holds is *show* state, so a
    /// client that expanded the group would be sending a selection that a
    /// second client's edit of that group had already made wrong. The command
    /// names the group and the daemon expands it — the same split
    /// `Command::PatchFixture` makes by carrying no channels.
    ///
    /// Refused when there is no such group, which is what makes `Group 9` on a
    /// show with eight of them a message rather than an empty selection.
    SelectGroup {
        /// The group.
        group_id: GroupId,
        /// How to combine with the current selection.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        mode: SelectionMode,
    },
    /// Apply a preset to the current selection.
    ApplyPreset {
        /// The preset to apply.
        preset_id: PresetId,
    },
    /// Advance the three-stage Clear.
    ClearProgrammer,
    /// Store the programmer contents into a cue.
    ///
    /// **The mode is the operator's and it travels here** (S39). S28 shipped
    /// this command without one on purpose: `prism_core::Programmer` merged
    /// unconditionally, and a client carrying a mode the daemon did not honour
    /// would have been describing an outcome that did not happen. What S28 did
    /// instead was *say so first* — `Query::StorePreview`, which now carries the
    /// chosen mode as well and answers with what **that** would cost.
    ///
    /// So the daemon never guesses, and the outcome does not depend on which
    /// client sent the command. See [`crate::StoreMode`] for what each of the
    /// three does.
    StoreCue {
        /// Target sequence, or `None` for the one the session has selected.
        ///
        /// **`Store Cue 5` names no cue list** (S40), and it means
        /// `Session::selectedSequence` — the field S39 added for exactly this
        /// line (`ARCHITECTURE_SPEC.md` §4.1). The daemon resolves it, because a
        /// client that read the session and filled the number in would be
        /// sending a command whose meaning had already moved on another screen.
        #[serde(default)]
        sequence_id: Option<SequenceId>,
        /// Cue number as typed, e.g. `1.5`.
        cue_number: String,
        /// How it combines with the cue that is already there.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        mode: StoreMode,
    },
    /// Store the programmer contents into a preset of a pool.
    ///
    /// **The mirror of [`Self::StoreCue`], with two differences that are both the
    /// type's rather than the session's.**
    ///
    /// It carries a name and a colour, because a preset has both and a cue's name
    /// is edited through [`Self::SetCueProperty`] instead. So a store with an
    /// empty programmer onto a preset that already exists is an ordinary relabel
    /// and is accepted; onto a preset that does not exist it is refused, because
    /// creating an empty preset gives an operator something that applies nothing.
    ///
    /// And which values are taken depends on the `pool`: a colour preset stores
    /// the colour values of the programmer and leaves the rest, where a cue takes
    /// everything. The bank an attribute is filed under is the **profile's**
    /// answer (`AttributeDef::featureGroup`) rather than the attribute name's, so
    /// this is a question only the daemon can settle.
    ///
    /// Added in **S28**, with [`Self::CreateSequence`], [`Self::SetCueProperty`],
    /// [`Self::DeleteCue`] and [`Self::AssignExecutor`]: before them
    /// `ApplyPreset` could apply a preset no interface could create.
    StorePreset {
        /// The preset number. Unique across pools — `prism_core::Show::
        /// store_preset` explains why, and [`Self::ApplyPreset`] is the reason.
        preset_id: PresetId,
        /// Which pool it is filed in, and which values it takes, or `None` for
        /// the bank the session has in force.
        ///
        /// **`Store Preset 1` names no pool** (S40), and the desk's answer is
        /// `Session::encoderBank` — the bank whose values are under the
        /// operator's hands at that moment, which is the one they mean. The
        /// alternative, making the line name a pool, would put a word in front
        /// of the commonest store on a console for no gain: the bank is already
        /// lit on the encoder bar. A preset that **exists** keeps its own pool
        /// instead, because a store onto preset 1 is a store into preset 1
        /// rather than a way of moving it between pools.
        #[serde(default)]
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        pool: Option<FeatureGroup>,
        /// Operator-facing name.
        name: String,
        /// Colour for the scribble strip, if one was chosen.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        color: Option<RgbColor>,
        /// How it combines with the preset that is already there (S39).
        ///
        /// The mirror of [`Self::StoreCue`]'s, and it has to exist for the same
        /// reason the preview does: `Query::StorePreview` can be asked about a
        /// preset in any of the three modes, and an answer describing an outcome
        /// no command can produce is exactly what S28 refused to ship.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        mode: StoreMode,
    },
    /// Store the programmer contents into a whole **sequence** (S39).
    ///
    /// A different act from [`Self::StoreCue`], which is why it is a different
    /// command: that one names a cue and this one does not. `Append` puts the
    /// look on a new cue at the highest number, `Override` makes the sequence be
    /// that look, and `Merge` writes it into every cue there is — see
    /// [`SequenceStoreMode`].
    ///
    /// **It creates the cue list when the number is free** (S40). S39 refused a
    /// store into a sequence that was not there and had `CreateSequence` beside
    /// it for the other case; S40 removed that command, because `Store Sequence
    /// 4` on the command line cannot know which of the two it is — the parser
    /// does not read the show, and never will (S26). So this is both: it makes
    /// the list if it has to, and stores into it. With an **empty programmer**
    /// on a number nobody has used, that is exactly the empty cue list
    /// `CreateSequence` used to make.
    StoreSequence {
        /// Target sequence. Created when the number is free.
        sequence_id: SequenceId,
        /// Operator-facing name, used only when the sequence is created.
        ///
        /// Renaming one is [`Self::Label`]: a store must not quietly rename a
        /// cue list somebody else named.
        name: String,
        /// What to do with the cue list that is there.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        mode: SequenceStoreMode,
    },
    /// Store the programmer's **selection** as a group (S40).
    ///
    /// The command `prism_core::Show::store_group` had been waiting for since
    /// S11: the method existed, `Effect::ReloadGroups` named it, and no command
    /// in `docs/IPC_PROTOCOL.md` §5 reached it — so a group could be read,
    /// selected and merged, and never made. S40 needs one, because `Copy Group 2
    /// Group 6` and `Delete Group 4` are lines about groups that have to exist
    /// first.
    ///
    /// It stores the **selection** and not the values: a group is a list of
    /// fixtures (`prism_domain::Group`), which is what makes `Group 3` a
    /// selection rather than a look. A look over the same fixtures is a preset.
    StoreGroup {
        /// The group number.
        group_id: GroupId,
        /// Operator-facing name, used only when the group is created.
        name: String,
        /// What to do with the group that is already there.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        mode: OverwriteMode,
    },
    /// Load a stored cue back into the programmer (S39).
    ///
    /// **Every attribute of that cue, with each `presetRef` kept.** A load that
    /// took the values and dropped the links would break every preset link in
    /// the cue the next time it was stored — invisibly, until somebody edited
    /// the preset and watched the cue not follow. `prism_domain::preset` has
    /// said since S1 that a linked part follows later edits of its preset, and
    /// `prism_core::Show::relink` (S28) is what makes that true.
    ///
    /// The values arrive as [`crate::ProgrammerValueSource::Recalled`], which is
    /// what that variant has been waiting for since S1: pulled back out of the
    /// show rather than set by hand or applied from a pool.
    ///
    /// It also sets the **update state** — see [`Self::Update`].
    EditCue {
        /// The sequence the cue is in, or `None` for the selected one — see
        /// [`Self::StoreCue`] for why the daemon resolves it rather than a
        /// client.
        #[serde(default)]
        sequence_id: Option<SequenceId>,
        /// The cue, by its number.
        cue_number: String,
    },
    /// Store the programmer back into the cue it was loaded from (S39).
    ///
    /// Carries nothing at all, because everything it needs is the desk's:
    /// `Session::editingCue` says which cue the programmer is editing, and the
    /// mode is [`crate::StoreMode::Override`] by definition — an Update that
    /// merged could never take a value *out* of the cue it is updating, which is
    /// the whole reason an operator loads one.
    ///
    /// Refused when nothing is being edited. It is a **show edit** and therefore
    /// undoable, unlike the playback actions beside it in
    /// `ARCHITECTURE_SPEC.md` §6.1.
    Update,
    /// Change one timing field of one cue.
    ///
    /// One field rather than a whole cue — see [`CueProperty`], which also
    /// explains why the number and the name are no longer among them. What a cue
    /// *does* is not among them either: values come from the programmer through
    /// [`Self::StoreCue`], the same rule that keeps channels out of
    /// [`Self::PatchFixture`].
    SetCueProperty {
        /// The sequence the cue is in, or `None` for the selected one.
        #[serde(default)]
        sequence_id: Option<SequenceId>,
        /// The cue, by the number it has **now**.
        cue_number: String,
        /// The field, and its new value.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        property: CueProperty,
    },
    /// Empty a place on the desk — S40's `Delete`.
    ///
    /// One command for six kinds of thing, and [`ObjectRef`] says why. What each
    /// one means is the pool's own answer and is documented on the
    /// `prism_core::Show` method behind it, but three of them are worth stating
    /// here because they are decisions rather than deletions:
    ///
    /// - **A cue does not renumber what is left.** The cues after it keep their
    ///   numbers, because a cue number is what an operator has written on a
    ///   running order and what a `Goto` names.
    /// - **A preset keeps its links.** A cue part that referenced it keeps the
    ///   value it was given and loses the link, which `Show::issues` reports —
    ///   editing every cue that used it would change light nobody asked to
    ///   change.
    /// - **An executor's *place* survives.** The row leaves the show, and the
    ///   eight strips of a page are `page * 8 + slot` arithmetic (**D7**) rather
    ///   than rows, so executor 1 is still there and is now empty. That is the
    ///   requirement read literally, and it is also the exact inverse of the
    ///   `AssignExecutor` that made the row — which is what lets an Oops put the
    ///   grid back as it was rather than leaving a slot behind with the master
    ///   and the button functions of a deleted executor still on it.
    ///
    /// The **last view** cannot be deleted (`SessionError::LastView`):
    /// `activeViewId` names a view from the first moment
    /// (`prism_core::SessionState::new`) and a session whose active view is not
    /// a view is a dangling reference. Deleting the *active* view is allowed and
    /// what the canvas then shows is the daemon's to decide.
    Delete {
        /// What to empty.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        target: ObjectRef,
    },
    /// Copy one place on the desk onto another — S40's `Copy`.
    ///
    /// `Copy Sequence 2 Sequence 6` and its five siblings. The source is left
    /// exactly as it was; what happens at the destination when something is
    /// already there is [`OverwriteMode`], and the interface asks before sending
    /// an `Override`.
    ///
    /// The two references must name the **same kind** of thing
    /// ([`ObjectRef::same_kind_as`]) — `Copy Cue 2 Group 6` is a line the parser
    /// will build and nothing can carry out, so it is refused rather than
    /// guessed at.
    Copy {
        /// What to copy.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        from: ObjectRef,
        /// Where to put it.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        to: ObjectRef,
        /// What to do when the destination is taken.
        mode: OverwriteMode,
    },
    /// Move one place on the desk to another — S40's `Move`.
    ///
    /// [`Self::Copy`] followed by [`Self::Delete`] of the source, except where
    /// that would be the wrong answer, and two of the six are exactly that:
    ///
    /// - **An executor swaps.** `Move Executor 1 Executor 5` puts 1's content on
    ///   5; if 5 held something it goes to 1, because a desk's faders are places
    ///   and an operator moving one is rearranging the grid rather than throwing
    ///   half of it away. The mode is not read.
    /// - **A view swaps, and its number does not travel.** S35 decided that a
    ///   view library's order **is** its numbers — `views` is keyed by number,
    ///   the View Selector Bar draws in number order and `Channel ◀▶` steps from
    ///   one number to the next — so moving a view exchanges the two views'
    ///   *contents* and leaves the numbers where they are. The cost is the one
    ///   S35 named and accepted: after a move, `SelectView 3` names a different
    ///   layout.
    ///
    /// A cue does **not** swap: `Move Cue 3 Cue 8` is a renumber, and a renumber
    /// onto a number that is taken is the merge-or-override question again.
    ///
    /// > **This replaced `MoveView`, which was relative** (S35: `Prev`/`Next`).
    /// > One absolute form covers both, because the bar knows its neighbour's
    /// > number and writes the line — which is the whole of §4.5. Two commands
    /// > for one act would have been the second grammar S40 exists to remove.
    Move {
        /// What to move.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        from: ObjectRef,
        /// Where to move it.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        to: ObjectRef,
        /// What to do when the destination is taken, where that is a question.
        mode: OverwriteMode,
    },
    /// Name one place on the desk — S40's `Label`.
    ///
    /// Six kinds of thing and one word, for [`ObjectRef`]'s reason. It replaced
    /// `RenameView` (S35) and `CueProperty::Name` (S28), which were the same act
    /// said twice; a sequence, a group and a preset could not be renamed at all
    /// before this, which `PROGRESS.md` §7 had been carrying out of S28 as *the
    /// correcting half is not covered*.
    ///
    /// An **executor** has no name of its own — what a strip shows is its
    /// sequence's — so `Label Executor 1` labels the cue list on it, and is
    /// refused when the slot is empty. That is one indirection and it is the one
    /// an operator means: the scribble strip is what they are trying to change.
    ///
    /// The name may be empty. Everything here is reached by its number, and an
    /// operator clearing a label is not making anything unreachable.
    Label {
        /// What to name.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        target: ObjectRef,
        /// The new name.
        name: String,
    },
    /// Jump a playback straight to a cue — S40's `Goto`.
    ///
    /// **The command S40 found missing at the bottom of the stack**: there was
    /// no `Goto` in this enum *and* none in `prism_engine::TickCommand`, so
    /// `Goto Cue 5` needed a message all the way down to the tick before the
    /// parser could send anything. What it does is [`Self::ExecutorGo`] without
    /// the stepping: the cue is entered with its own fade, delay and trigger,
    /// exactly as if the list had arrived there.
    ///
    /// The cue is named by **number**, and the daemon resolves it to the index
    /// the tick works in — a client that sent an index would be reading a cue
    /// list it may be one delta behind on.
    ///
    /// Not undoable, like every other playback action (`ARCHITECTURE_SPEC.md`
    /// §6.1).
    Goto {
        /// Which playback.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        target: PlaybackTarget,
        /// The cue, by the number an operator typed.
        cue_number: String,
    },
    /// Start a playback at the first cue of its list — S40's `On`.
    ///
    /// The command form of `ExecutorButtonFunction::On`, which until S40 could
    /// only be reached by *pressing a key of an executor* — so `On Sequence 1`
    /// had nothing to send. A second press does not restart a list that is
    /// already running (`prism_engine::CuePlayer::on`).
    ///
    /// [`Self::ExecutorButton`] is still the right command for a strip, and this
    /// is still not the same thing: that one says *which key went down* and lets
    /// the executor decide, which is **D3** for playback.
    ExecutorOn {
        /// Which playback.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        target: PlaybackTarget,
    },
    /// Put a sequence on an executor, or take one off.
    ///
    /// The command that makes a cue list playable *from an interface*: until S28
    /// an executor could be given a sequence only by a show file somebody else
    /// had written, so `ExecutorGo` had nothing to reach. An empty slot gains an
    /// executor with the desk's defaults — see `prism_core::Show::
    /// assign_executor`, which owns them, because a client choosing what a fader
    /// and four buttons do would be authoring show content.
    AssignExecutor {
        /// The executor slot, `page * 8 + slot` (**D7**).
        executor_id: ExecutorId,
        /// The sequence to put on it, or `None` to clear the slot's sequence.
        sequence_id: Option<SequenceId>,
    },
    /// Step a playback.
    ///
    /// **The target grew in S40** and the reason is in [`PlaybackTarget`]: until
    /// then every playback command named an executor, so `Go+ Sequence 1` for a
    /// cue list on no fader had no representation at all. A strip still sends
    /// [`PlaybackTarget::Executor`] and nothing about the latency path
    /// (`ARCHITECTURE_SPEC.md` §4.3) changed — resolving that variant is the
    /// identity.
    ExecutorGo {
        /// Which playback.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        target: PlaybackTarget,
        /// Which way to step.
        direction: GoDirection,
    },
    /// Stop a playback.
    ExecutorOff {
        /// Which playback.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        target: PlaybackTarget,
    },
    /// Press or release one of an executor's buttons.
    ///
    /// **The command that lets the executor decide what a press means** — the
    /// gap `docs/MCU_MAPPING.md` §4.2.1 recorded from the binding table and S26
    /// met again from the interface, closed in S34. `ExecutorGo` and
    /// `ExecutorOff` say *what to do*; this says *what was pressed*, and
    /// `prism_core::Show::apply` resolves it against that executor's own
    /// `button_functions`. That is the whole of the difference: `Toggle` is
    /// resolved against `is_active` by the daemon, which owns it, and never by a
    /// client, which would race a second client doing the same.
    ///
    /// `pressed` is what `Flash` needs: a momentary function has a down and an
    /// up, and the up is not a second press. Functions that are not momentary
    /// act on the down and ignore the up.
    ExecutorButton {
        /// Target executor.
        executor_id: ExecutorId,
        /// Which button.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        button: ExecutorButtonRef,
        /// Whether the button went down (`true`) or came up (`false`).
        pressed: bool,
    },
    /// Move an executor's fader.
    ///
    /// **What the fader does is the executor's own setting** — `Master`,
    /// `Speed`, `XFade` or nothing (`ExecutorFaderFunction`). One command for
    /// all four, for the reason `docs/MCU_MAPPING.md` §4.1 gives the main fader
    /// a single row: a surface and a screen both move *the fader*, and which of
    /// the four it is belongs to the show. The name is the one it has had since
    /// S1 and is kept so a saved profile and a recorded script still parse.
    SetExecutorMaster {
        /// Target executor.
        executor_id: ExecutorId,
        /// New position, `0..=65535`.
        level: u16,
    },
    /// Patch a fixture into a universe.
    ///
    /// Carries the start address only, not the individual DMX channels. The
    /// channel layout is derived: it follows from the [`crate::FixtureType`] that
    /// `type_id` names, and `prism-engine` resolves it once at patch time (S4) so
    /// the tick needs no lookup. Putting the resolved channels in the command
    /// would make a client compute state the daemon must then accept, which
    /// decision **D3** exists to prevent, and would duplicate a derived value
    /// that then has to be kept in step with the profile.
    ///
    /// The risk that motivates the question is real but lives elsewhere: if the
    /// profile library changes under a saved show, patched fixtures silently
    /// change meaning. The fix for that is for the show file to **embed** the
    /// fixture types it uses rather than reference an external library — a
    /// requirement on the show model in S11, not on this command.
    PatchFixture {
        /// Fixture number to assign.
        id: FixtureId,
        /// Operator-facing name.
        name: String,
        /// Key of the fixture type to instantiate.
        type_id: String,
        /// Universe to patch into.
        universe: UniverseId,
        /// Start address, `1..=512`.
        address: u16,
    },
    /// Take a fixture out of the patch.
    ///
    /// **Does not cascade** into groups, presets or cues: a show outlives the
    /// rig it was written on, and deleting an operator's stored looks because a
    /// light came out of the rig for one show would be worse than leaving them
    /// dangling. `prism_core::Show::issues` reports what now points at nothing.
    ///
    /// Added in **S27**, with [`Self::RenumberFixture`], because a patch that
    /// can only ever be added to is not one an operator can correct — and
    /// because the interface's only other way to change a fixture *number* would
    /// have been to unpatch and repatch as two commands, which is a rig with a
    /// hole in it if the second one is refused.
    UnpatchFixture {
        /// The fixture to remove.
        id: FixtureId,
    },
    /// Give a patched fixture a different number.
    ///
    /// One command rather than two, and that is the whole reason it exists: the
    /// number is the key the patch is filed under, so changing it is a remove
    /// and an insert — and a client that sent those separately would leave the
    /// rig without that fixture for as long as the round trip took, or for ever
    /// if the second half were refused. Everything else about the fixture,
    /// including its position, rotation and inverts, comes with it.
    RenumberFixture {
        /// The fixture as it is numbered now.
        id: FixtureId,
        /// The number it should have. Refused when something is already there:
        /// two fixtures cannot share a number, and silently replacing the other
        /// one would delete a light nobody asked to delete.
        to: FixtureId,
    },
    /// Embed one of the desk's built-in profiles into the show.
    ///
    /// Carries the **key only**, for the same reason
    /// [`Self::PatchFixture`] carries no channels: the profile is
    /// `prism_core::library`'s, the daemon copies it into the show, and a client
    /// that sent a whole [`crate::FixtureType`] would be authoring show content
    /// that the daemon would then have to validate and accept.
    ///
    /// A show **embeds** the profiles it uses rather than referencing a library
    /// (S11), so this is a copy at a moment in time: a later desk with a
    /// different library opens the show unchanged.
    EmbedFixtureType {
        /// The library key, e.g. `generic.rgbw.par`.
        type_id: String,
    },
    /// Undo the last undoable command.
    Oops,
    /// Redo the last undone command.
    Redo,
    /// Write the show to disk.
    SaveShow,
    /// Switch the canvas to a stored view.
    SelectView {
        /// The view to activate.
        view_id: ViewId,
    },
    /// Store the current canvas as a view.
    StoreView {
        /// The view number to write.
        view_id: ViewId,
        /// Name for the view.
        name: String,
    },
    /// Open a window on the canvas.
    OpenWindow {
        /// Which window to open.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        window: WindowType,
        /// Window-specific parameters, e.g. which preset pool.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "proptest::option::of(crate::arb::small_map(2))")
        )]
        #[ts(optional)]
        params: Option<BTreeMap<String, JsonValue>>,
    },
    /// Close an open window.
    CloseWindow {
        /// The window to close.
        instance_id: WindowInstanceId,
    },
    /// Bring a window to the front.
    FocusWindow {
        /// The window to focus.
        instance_id: WindowInstanceId,
    },
    /// Move and resize an open window.
    ///
    /// **Not one of `ARCHITECTURE_SPEC.md` §4.4's eleven**, and the reason is
    /// that §4.4 lists what the *console* issues: an X-Touch opens and closes
    /// windows, it does not drag them. But §4.1 puts `x`, `y`, `w` and `h` in
    /// the session, so a window dragged on one screen has to move on every
    /// other one — and a client that kept the position to itself would be
    /// holding session state locally, which is the thing D11 exists to prevent.
    /// S25 found the gap; without this command a canvas has no honest way to be
    /// dragged at all. See `crate::WindowInstance` for the units: they are
    /// canvas units, which each client scales to its own screen.
    ///
    /// The four coordinates are guarded against NaN and infinity in **both**
    /// directions, like every other `f64` in this crate, so a non-finite
    /// coordinate is refused at the decoder rather than written into a session
    /// that then cannot be saved.
    PlaceWindow {
        /// The window to move.
        instance_id: WindowInstanceId,
        /// New left edge.
        #[serde(with = "crate::finite")]
        #[ts(as = "f64")]
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::finite_f64()")
        )]
        x: f64,
        /// New top edge.
        #[serde(with = "crate::finite")]
        #[ts(as = "f64")]
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::finite_f64()")
        )]
        y: f64,
        /// New width.
        #[serde(with = "crate::finite")]
        #[ts(as = "f64")]
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::finite_f64()")
        )]
        w: f64,
        /// New height.
        #[serde(with = "crate::finite")]
        #[ts(as = "f64")]
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::finite_f64()")
        )]
        h: f64,
    },
    /// Page the fader bank.
    SetExecutorPage {
        /// New page number.
        page: u32,
    },
    /// Select the executor the main fader and transport act on.
    SelectExecutor {
        /// The executor to select.
        executor_id: ExecutorId,
    },
    /// Select the sequence a store with no cue list named goes into (S39).
    ///
    /// **S39's decision, and it was a decision.** `ARCHITECTURE_SPEC.md` §4.1
    /// had no *selected sequence* until this command existed, and S28 marked the
    /// absence rather than inventing one: the cue sheet followed the selected
    /// executor's sequence, which needed nothing added to the session. §4.4
    /// named S39 as the session that would settle it, and it settles it the
    /// other way — a sequence is selected in its own right, because otherwise
    /// `Store Cue 5` typed with no executor selected means nothing at all, and a
    /// cue list nobody has put on a fader cannot be edited without occupying a
    /// playback slot to do it.
    ///
    /// It is session state and not client state for `SelectExecutor`'s reason:
    /// two screens must not disagree about which cue list is in force, and the
    /// console has to be able to say it (S40's command line).
    SelectSequence {
        /// The sequence to select. It is **not** validated against the show,
        /// exactly as [`Self::SelectExecutor`] is not: the session applier has
        /// no show, and a selection naming a sequence that has gone reads as
        /// nothing rather than as a refusal.
        sequence_id: SequenceId,
    },
    /// Switch the encoder bank.
    SetEncoderBank {
        /// The feature group to switch to.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        group: FeatureGroup,
    },
    /// Page the programmer.
    SetProgrammerPage {
        /// New page number.
        page: u32,
    },
    /// Move the programmer parameter the jog wheel turns.
    SelectProgrammerParam {
        /// Which way to move.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        direction: ParamDirection,
    },
    /// Type into the command line.
    CommandLineInput {
        /// The text entered.
        text: String,
    },

    // ---- The machine's own rig (S33) ----
    /// Adds an output to **this machine's** rig — S33.
    ///
    /// The number is the operator's, like a fixture number: `Output 3` names
    /// this row afterwards. A number that is taken is refused rather than
    /// overwritten, because an add that replaced a running node would take a
    /// universe off stage without saying so.
    ///
    /// It is neither a show command nor a session command — see
    /// [`Self::is_machine_command`].
    AddOutput {
        /// The output to add, with the number it is to have.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        output: OutputInstance,
    },
    /// Changes one thing about a configured output — S33.
    ///
    /// See [`OutputChange`] for why it is one field and not the whole row.
    ConfigureOutput {
        /// Which output.
        id: OutputId,
        /// What to change.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        change: OutputChange,
    },
    /// Takes an output out of the rig — S33.
    ///
    /// Its driver stops and its line goes quiet; every other output carries on
    /// without a missed frame, which is asserted rather than assumed.
    RemoveOutput {
        /// Which output.
        id: OutputId,
    },
    /// Stops an output sending, or starts it again — S33.
    ///
    /// A disabled output keeps its whole configuration, which is what an
    /// operator wants of a node that is being worked on: the alternative is
    /// deleting the row and typing it back in afterwards.
    SetOutputEnabled {
        /// Which output.
        id: OutputId,
        /// Whether it should be sending.
        enabled: bool,
    },

    // ---- The machine's own control surface (S36) ----
    /// Names the MIDI port this desk's control surface is on — S36.
    ///
    /// A **machine** command for the same reason the four above it are: the
    /// X-Touch is plugged into *this building's* rack, and a show copied to
    /// another hall on a stick must not bring a port name with it any more than
    /// it brings the cabling. So it lands in `prism_core::MachineConfig` beside
    /// the desk identity and the output patch, and both of the other appliers
    /// refuse it by name.
    ///
    /// The port is a **name** rather than an index, because an index renumbers
    /// itself when somebody moves a plug and a name does not. What a name has
    /// to survive is the decoration each platform puts round it, which is
    /// `prism_midi::selects`' business and not the protocol's — the protocol
    /// carries exactly what the operator picked out of
    /// [`crate::Answer::MidiPorts`].
    ///
    /// `None` is *no surface*, and it is an ordinary state rather than an
    /// error: a desk being programmed on a laptop has no X-Touch, and a
    /// settings window has to be able to say so.
    ///
    /// **A port that is not there is still accepted.** The device may be
    /// switched off, or the show may be being prepared a week before the get-in
    /// — the same reason an output row is not validated against a cable being
    /// plugged in (S33). The daemon warns, keeps trying, and starts.
    SetSurfacePort {
        /// The port name, or `None` for no surface at all.
        port: Option<String>,
    },
}

impl Command {
    /// Whether this command acts on session state rather than on the show.
    ///
    /// `ARCHITECTURE_SPEC.md` §4.4's twelve, plus [`Self::PlaceWindow`], which
    /// is thirteenth because §4.4 lists what the *console* issues and a canvas
    /// is not a console.
    ///
    /// [`Self::SelectSequence`] is on §4.4's list since S39, because a console
    /// issues it: `Sequence 5` on the command line is how an operator picks a
    /// cue list without a pointer.
    ///
    /// # Four of them are session commands only sometimes, and that is S40's
    ///
    /// [`Self::Delete`], [`Self::Copy`], [`Self::Move`] and [`Self::Label`] name
    /// a *thing* rather than a model, and one of the six things they can name —
    /// a view — lives in the session (§4.1). So which applier owns one of these
    /// is a property of its target, and this predicate reads it.
    ///
    /// That is new in this codebase and it is worth being plain about the cost:
    /// `ShowFile::apply` routes on this predicate, so a `Delete` is a session
    /// command or a show command depending on its payload, and
    /// [`Self::is_undoable`] follows it — deleting a view is not undoable, for
    /// §6.1's reason that an Oops must not pull a window out from under an
    /// operator, and deleting a cue is. The alternative was twenty-four
    /// commands whose only difference is which pool they index, and
    /// [`ObjectRef`] has the rest of that argument.
    ///
    /// `Copy` and `Move` read the **source**. A pair naming two different kinds
    /// of thing is refused by whichever applier gets it, which is why it does
    /// not matter which one that is.
    #[must_use]
    pub const fn is_session_command(&self) -> bool {
        match self {
            Self::SelectView { .. }
            | Self::StoreView { .. }
            | Self::OpenWindow { .. }
            | Self::CloseWindow { .. }
            | Self::FocusWindow { .. }
            | Self::PlaceWindow { .. }
            | Self::SetExecutorPage { .. }
            | Self::SelectExecutor { .. }
            | Self::SelectSequence { .. }
            | Self::SetEncoderBank { .. }
            | Self::SetProgrammerPage { .. }
            | Self::SelectProgrammerParam { .. }
            | Self::CommandLineInput { .. } => true,
            Self::Delete { target } | Self::Label { target, .. } => target.is_session_object(),
            Self::Copy { from, .. } | Self::Move { from, .. } => from.is_session_object(),
            _ => false,
        }
    }

    /// Whether this command configures **the machine** rather than the show or
    /// the session — S33.
    ///
    /// # A third applier, and why there had to be one
    ///
    /// The output patch is neither. It is not the show's, because a show copied
    /// to a second machine on a stick must not bring the first hall's cabling
    /// with it — the same argument `prism_core::desk` makes for the sACN CID,
    /// and the same reason a `.prism` file has no table for either. And it is
    /// not the session's, because the session is *persisted with the show*
    /// (`ARCHITECTURE_SPEC.md` §4.1) and would carry the cabling by the same
    /// route.
    ///
    /// So these go to `prism_core::MachineConfig::apply`, and both of the
    /// other two appliers refuse them by name. `ShowFile::apply` never sees one:
    /// the daemon routes on this predicate first, exactly as it routes on
    /// [`Self::is_session_command`] second.
    ///
    /// **None of them is undoable**, and that follows from where they live
    /// rather than from a separate decision: the Oops journal is the *show's*,
    /// it is cleared when a show is loaded, and an undo that re-addressed a
    /// node would move light on a stage while somebody was driving it — which is
    /// §6.1's rule for playback actions, met by another road.
    ///
    /// **Five since S36**, which put the control surface's MIDI port in the same
    /// place for the same reason: the desk in the rack belongs to the building,
    /// not to the show. S33 carried that out of itself in as many words — *a
    /// later session that wants anything else about this building puts it here*
    /// — and [`Self::SetSurfacePort`] is the session that did.
    #[must_use]
    pub const fn is_machine_command(&self) -> bool {
        matches!(
            self,
            Self::AddOutput { .. }
                | Self::ConfigureOutput { .. }
                | Self::RemoveOutput { .. }
                | Self::SetOutputEnabled { .. }
                | Self::SetSurfacePort { .. }
        )
    }

    /// Whether applying this command should push an entry onto the Oops journal.
    ///
    /// `ARCHITECTURE_SPEC.md` §6.1: playback actions and every session command
    /// are deliberately excluded, so undo during a running show neither changes
    /// light the operator is driving nor pulls windows out from under them.
    /// `Oops`, `Redo` and `SaveShow` are excluded because they are not show
    /// mutations in the first place. **Every machine command is excluded too**
    /// (S33): the journal belongs to the show and is cleared when one is loaded,
    /// so an Oops that reached the venue's cabling would take back a change the
    /// show it is journaling knows nothing about — and it would move light on a
    /// stage, which is the rule the playback actions are excluded by.
    #[must_use]
    pub const fn is_undoable(&self) -> bool {
        !matches!(
            self,
            Self::ExecutorGo { .. }
                | Self::ExecutorOff { .. }
                | Self::ExecutorOn { .. }
                | Self::ExecutorButton { .. }
                | Self::Goto { .. }
                | Self::SetExecutorMaster { .. }
                | Self::Oops
                | Self::Redo
                | Self::SaveShow
        ) && !self.is_session_command()
            && !self.is_machine_command()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        AttributeType, Command, CueProperty, ExecutorButtonRef, ExecutorId, FeatureGroup,
        FixtureId, GoDirection, GroupId, JsonValue, ObjectRef, OutputChange, OutputId,
        OutputInstance, OutputKind, OverwriteMode, ParamDirection, PlaybackTarget, PresetId,
        RgbColor, SelectionMode, SequenceId, SequenceStoreMode, StoreMode, UniverseId, ViewId,
        WindowInstanceId, WindowType,
    };
    use std::collections::BTreeMap;

    #[test]
    fn commands_are_internally_tagged_with_t() {
        let command = Command::SelectFixtures {
            ids: vec![FixtureId::new(1), FixtureId::new(2)],
            mode: SelectionMode::Add,
        };
        assert_eq!(
            serde_json::to_string(&command).unwrap(),
            r#"{"t":"SelectFixtures","ids":[1,2],"mode":"Add"}"#
        );
    }

    #[test]
    fn a_command_without_arguments_carries_only_its_tag() {
        assert_eq!(
            serde_json::to_string(&Command::ClearProgrammer).unwrap(),
            r#"{"t":"ClearProgrammer"}"#
        );
        assert_eq!(
            serde_json::to_string(&Command::Oops).unwrap(),
            r#"{"t":"Oops"}"#
        );
    }

    #[test]
    fn set_attribute_carries_a_signed_value_so_encoders_can_turn_both_ways() {
        let command = Command::SetAttribute {
            attribute: AttributeType::Tilt,
            value: -128,
            relative: true,
        };
        assert_eq!(
            serde_json::to_string(&command).unwrap(),
            r#"{"t":"SetAttribute","attribute":"Tilt","value":-128,"relative":true}"#
        );
    }

    #[test]
    fn open_window_omits_params_when_there_are_none() {
        assert_eq!(
            serde_json::to_string(&Command::OpenWindow {
                window: WindowType::Patch,
                params: None,
            })
            .unwrap(),
            r#"{"t":"OpenWindow","window":"Patch"}"#
        );
        assert_eq!(
            serde_json::to_string(&Command::OpenWindow {
                window: WindowType::PresetPool,
                params: Some(BTreeMap::from([(
                    "pool".to_owned(),
                    JsonValue::String("Color".to_owned())
                )])),
            })
            .unwrap(),
            r#"{"t":"OpenWindow","window":"PresetPool","params":{"pool":"Color"}}"#
        );
    }

    #[test]
    fn a_missing_params_field_deserialises_to_none() {
        let command: Command =
            serde_json::from_str(r#"{"t":"OpenWindow","window":"Patch"}"#).unwrap();
        assert_eq!(
            command,
            Command::OpenWindow {
                window: WindowType::Patch,
                params: None,
            }
        );
    }

    #[test]
    fn every_command_from_the_protocol_specification_exists() {
        // docs/IPC_PROTOCOL.md §5, in order. Compiling this list is the check.
        let commands = [
            Command::SelectFixtures {
                ids: vec![],
                mode: SelectionMode::Set,
            },
            Command::SetAttribute {
                attribute: AttributeType::Dimmer,
                value: 0,
                relative: false,
            },
            Command::SelectGroup {
                group_id: GroupId::new(1),
                mode: SelectionMode::Set,
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
            // S39's three: a store into a whole cue list, a cue loaded back into
            // the programmer, and the store that puts it back.
            Command::StoreSequence {
                sequence_id: SequenceId::new(1),
                name: "Act 1".to_owned(),
                mode: SequenceStoreMode::Append,
            },
            Command::StoreGroup {
                group_id: GroupId::new(1),
                name: "Front wash".to_owned(),
                mode: OverwriteMode::Merge,
            },
            Command::EditCue {
                sequence_id: Some(SequenceId::new(1)),
                cue_number: "1".to_owned(),
            },
            Command::Update,
            // S40's playback vocabulary: four verbs and three targets.
            Command::ExecutorGo {
                target: PlaybackTarget::of_executor(ExecutorId::new(0)),
                direction: GoDirection::Next,
            },
            Command::ExecutorOff {
                target: PlaybackTarget::of_sequence(SequenceId::new(1)),
            },
            Command::ExecutorOn {
                target: PlaybackTarget::Selected,
            },
            Command::Goto {
                target: PlaybackTarget::Selected,
                cue_number: "5".to_owned(),
            },
            Command::ExecutorButton {
                executor_id: ExecutorId::new(0),
                button: ExecutorButtonRef::Slot { index: 3 },
                pressed: true,
            },
            Command::SetExecutorMaster {
                executor_id: ExecutorId::new(0),
                level: 0,
            },
            Command::PatchFixture {
                id: FixtureId::new(1),
                name: "PAR 1".to_owned(),
                type_id: "generic.rgbw.par".to_owned(),
                universe: UniverseId::new(1),
                address: 1,
            },
            Command::UnpatchFixture {
                id: FixtureId::new(1),
            },
            Command::RenumberFixture {
                id: FixtureId::new(1),
                to: FixtureId::new(2),
            },
            Command::EmbedFixtureType {
                type_id: "generic.rgbw.par".to_owned(),
            },
            Command::StorePreset {
                preset_id: PresetId::new(4),
                pool: Some(FeatureGroup::Color),
                name: "Deep blue".to_owned(),
                color: Some(RgbColor { r: 0, g: 0, b: 255 }),
                mode: StoreMode::Override,
            },
            Command::SetCueProperty {
                sequence_id: Some(SequenceId::new(1)),
                cue_number: "1".to_owned(),
                property: CueProperty::FadeIn { seconds: 3.0 },
            },
            // S40's four verbs, one target apiece out of the six there are.
            Command::Delete {
                target: ObjectRef::Cue {
                    sequence_id: Some(SequenceId::new(1)),
                    cue_number: "1".to_owned(),
                },
            },
            Command::Copy {
                from: ObjectRef::Sequence {
                    sequence_id: SequenceId::new(2),
                },
                to: ObjectRef::Sequence {
                    sequence_id: SequenceId::new(6),
                },
                mode: OverwriteMode::Merge,
            },
            Command::Move {
                from: ObjectRef::Executor {
                    executor_id: ExecutorId::new(1),
                },
                to: ObjectRef::Executor {
                    executor_id: ExecutorId::new(5),
                },
                mode: OverwriteMode::Merge,
            },
            Command::Label {
                target: ObjectRef::Group {
                    group_id: GroupId::new(3),
                },
                name: "Front wash".to_owned(),
            },
            Command::AssignExecutor {
                executor_id: ExecutorId::new(0),
                sequence_id: Some(SequenceId::new(1)),
            },
            Command::Oops,
            Command::Redo,
            Command::SaveShow,
            Command::SelectView {
                view_id: ViewId::new(1),
            },
            Command::StoreView {
                view_id: ViewId::new(1),
                name: "Programming".to_owned(),
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
                w: 640.0,
                h: 480.0,
            },
            Command::SetExecutorPage { page: 0 },
            Command::SelectExecutor {
                executor_id: ExecutorId::new(0),
            },
            Command::SelectSequence {
                sequence_id: SequenceId::new(1),
            },
            Command::SetEncoderBank {
                group: FeatureGroup::Color,
            },
            Command::SetProgrammerPage { page: 0 },
            Command::SelectProgrammerParam {
                direction: ParamDirection::Next,
            },
            Command::CommandLineInput {
                text: "1 thru 4 at full".to_owned(),
            },
            // S33's four. Neither the show's nor the session's — see
            // `Command::is_machine_command`.
            Command::AddOutput {
                output: OutputInstance::new(
                    OutputId::new(1),
                    "Hall dimmers",
                    OutputKind::OpenDmx { serial: None },
                    [UniverseId::new(1)],
                ),
            },
            Command::ConfigureOutput {
                id: OutputId::new(1),
                change: OutputChange::Universes {
                    universes: vec![UniverseId::new(2)],
                },
            },
            Command::RemoveOutput {
                id: OutputId::new(1),
            },
            Command::SetOutputEnabled {
                id: OutputId::new(1),
                enabled: false,
            },
            // S36's one, and it is a machine command for S33's reason: the desk
            // in the rack belongs to the building.
            Command::SetSurfacePort {
                port: Some("X-Touch".to_owned()),
            },
        ];
        assert_eq!(commands.len(), 48);

        // Every command must survive the wire, and the tag must be stable.
        for command in commands {
            let json = serde_json::to_string(&command).unwrap();
            assert!(json.starts_with(r#"{"t":""#), "{json}");
            let back: Command = serde_json::from_str(&json).unwrap();
            assert_eq!(back, command);
        }
    }

    /// S33's four are a group of their own, and the three predicates partition
    /// the protocol: every command is the show's, the session's or the
    /// machine's, and never two of those.
    #[test]
    fn the_machine_commands_are_neither_the_shows_nor_the_sessions() {
        let machine = [
            Command::AddOutput {
                output: OutputInstance::new(
                    OutputId::new(2),
                    "Stage left node",
                    OutputKind::ArtNet {
                        nodes: vec!["10.0.0.9:6454".parse().unwrap()],
                        sync: false,
                        ports: Vec::new(),
                    },
                    [UniverseId::new(5)],
                ),
            },
            Command::ConfigureOutput {
                id: OutputId::new(2),
                change: OutputChange::Name {
                    name: "Stage right node".to_owned(),
                },
            },
            Command::ConfigureOutput {
                id: OutputId::new(2),
                change: OutputChange::Kind {
                    kind: OutputKind::Mock,
                },
            },
            Command::RemoveOutput {
                id: OutputId::new(2),
            },
            Command::SetOutputEnabled {
                id: OutputId::new(2),
                enabled: true,
            },
            Command::SetSurfacePort {
                port: Some("2- X-Touch".to_owned()),
            },
            // No surface at all is an ordinary configuration, not an absent
            // one: a show programmed on a laptop has no X-Touch.
            Command::SetSurfacePort { port: None },
        ];
        for command in &machine {
            assert!(command.is_machine_command(), "{command:?}");
            assert!(!command.is_session_command(), "{command:?}");
            // The journal is the show's and is cleared when a show is loaded, so
            // an Oops must not reach the venue's cabling — and it must not move
            // light on a stage either (§6.1).
            assert!(!command.is_undoable(), "{command:?}");
        }

        // And nothing else is one. A command that grew a machine meaning
        // without saying so here would be routed to the wrong applier.
        for command in [
            Command::ClearProgrammer,
            Command::SelectView {
                view_id: ViewId::new(1),
            },
            Command::SaveShow,
        ] {
            assert!(!command.is_machine_command(), "{command:?}");
        }
    }

    #[test]
    fn session_commands_are_the_architecture_spec_list_plus_the_ones_a_screen_needs() {
        // ARCHITECTURE_SPEC.md §4.4's twelve, plus `PlaceWindow`, which a
        // *screen* needs and a console cannot issue: §4.1 puts a window's
        // position and size in the session and an X-Touch never drags one. Each
        // of them is a command or it is client-local state pretending not to be.
        //
        // S40's four generic verbs join them **when they name a view**, because
        // §4.1 puts the view library in the session too.
        let session_commands = [
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
                x: 1.0,
                y: 2.0,
                w: 3.0,
                h: 4.0,
            },
            Command::SetExecutorPage { page: 0 },
            Command::SelectExecutor {
                executor_id: ExecutorId::new(0),
            },
            // S39's, and the one of the four that a console *can* issue: the
            // command line's `Sequence 5`.
            Command::SelectSequence {
                sequence_id: SequenceId::new(1),
            },
            Command::SetEncoderBank {
                group: FeatureGroup::Dimmer,
            },
            Command::SetProgrammerPage { page: 0 },
            Command::SelectProgrammerParam {
                direction: ParamDirection::Prev,
            },
            Command::CommandLineInput {
                text: String::new(),
            },
            // S40's four, each with a **view** as its target. The same four
            // commands naming anything else are show commands - see
            // `Command::is_session_command`, which is why this list is the
            // interesting half rather than the whole of the predicate.
            Command::Delete {
                target: ObjectRef::View {
                    view_id: ViewId::new(1),
                },
            },
            Command::Label {
                target: ObjectRef::View {
                    view_id: ViewId::new(1),
                },
                name: "Busking".to_owned(),
            },
            Command::Copy {
                from: ObjectRef::View {
                    view_id: ViewId::new(1),
                },
                to: ObjectRef::View {
                    view_id: ViewId::new(2),
                },
                mode: OverwriteMode::Merge,
            },
            Command::Move {
                from: ObjectRef::View {
                    view_id: ViewId::new(1),
                },
                to: ObjectRef::View {
                    view_id: ViewId::new(3),
                },
                mode: OverwriteMode::Merge,
            },
        ];
        assert_eq!(session_commands.len(), 17);
        for command in session_commands {
            assert!(command.is_session_command(), "{command:?}");
        }
        assert!(!Command::ClearProgrammer.is_session_command());
    }

    /// **The same verb is a show command when it names anything but a view.**
    /// S40's cost, asserted rather than described: `ARCHITECTURE_SPEC.md` §4.1
    /// puts the view library in the session and everything else in the show, so
    /// which applier owns a `Delete` is a property of its payload.
    #[test]
    fn the_four_generic_verbs_follow_their_target_into_the_show() {
        let show_targets = [
            ObjectRef::Sequence {
                sequence_id: SequenceId::new(1),
            },
            ObjectRef::Cue {
                sequence_id: None,
                cue_number: "1".to_owned(),
            },
            ObjectRef::Group {
                group_id: GroupId::new(1),
            },
            ObjectRef::Preset {
                preset_id: PresetId::new(1),
            },
            ObjectRef::Executor {
                executor_id: ExecutorId::new(1),
            },
        ];
        for target in show_targets {
            let commands = [
                Command::Delete {
                    target: target.clone(),
                },
                Command::Label {
                    target: target.clone(),
                    name: String::new(),
                },
                Command::Copy {
                    from: target.clone(),
                    to: target.clone(),
                    mode: OverwriteMode::Merge,
                },
                Command::Move {
                    from: target.clone(),
                    to: target.clone(),
                    mode: OverwriteMode::Override,
                },
            ];
            for command in commands {
                assert!(!command.is_session_command(), "{command:?}");
                // And therefore undoable: a show edit is, a session edit is not.
                assert!(command.is_undoable(), "{command:?}");
            }
        }
        assert!(
            !Command::Delete {
                target: ObjectRef::View {
                    view_id: ViewId::new(1)
                }
            }
            .is_undoable()
        );
    }

    /// The playback verbs are the exclusion `ARCHITECTURE_SPEC.md` §6.1 names:
    /// an Oops must not change light the operator is currently driving. `Goto`
    /// and `ExecutorOn` joined them in S40.
    #[test]
    fn no_playback_action_is_undoable() {
        let playback = [
            Command::ExecutorGo {
                target: PlaybackTarget::Selected,
                direction: GoDirection::Next,
            },
            Command::ExecutorOff {
                target: PlaybackTarget::Selected,
            },
            Command::ExecutorOn {
                target: PlaybackTarget::Selected,
            },
            Command::Goto {
                target: PlaybackTarget::Selected,
                cue_number: "5".to_owned(),
            },
            Command::ExecutorButton {
                executor_id: ExecutorId::new(1),
                button: ExecutorButtonRef::Slot { index: 0 },
                pressed: true,
            },
            Command::SetExecutorMaster {
                executor_id: ExecutorId::new(1),
                level: 0,
            },
        ];
        for command in playback {
            assert!(!command.is_undoable(), "{command:?}");
        }
    }

    /// A pair naming two different kinds of thing is a line the parser will
    /// build and nothing can carry out - S40's rule that the client decides what
    /// was *asked for* and the daemon decides what is.
    #[test]
    fn a_copy_knows_whether_its_two_ends_are_the_same_kind_of_thing() {
        let cue = ObjectRef::Cue {
            sequence_id: None,
            cue_number: "1".to_owned(),
        };
        let other_cue = ObjectRef::Cue {
            sequence_id: Some(SequenceId::new(4)),
            cue_number: "9".to_owned(),
        };
        let group = ObjectRef::Group {
            group_id: GroupId::new(1),
        };
        assert!(cue.same_kind_as(&other_cue));
        assert!(!cue.same_kind_as(&group));
        assert!(group.same_kind_as(&group));
    }

    /// Refusals are shown to an operator, so a reference has to read as the
    /// words they typed.
    #[test]
    fn an_object_reference_reads_as_the_line_that_named_it() {
        assert_eq!(
            ObjectRef::Sequence {
                sequence_id: SequenceId::new(4)
            }
            .to_string(),
            "sequence 4"
        );
        assert_eq!(
            ObjectRef::Cue {
                sequence_id: None,
                cue_number: "1.5".to_owned()
            }
            .to_string(),
            "cue 1.5"
        );
        assert_eq!(
            ObjectRef::Cue {
                sequence_id: Some(SequenceId::new(2)),
                cue_number: "1.5".to_owned()
            }
            .to_string(),
            "sequence 2 cue 1.5"
        );
        assert_eq!(
            ObjectRef::Group {
                group_id: GroupId::new(3)
            }
            .to_string(),
            "group 3"
        );
        assert_eq!(
            ObjectRef::Preset {
                preset_id: PresetId::new(3)
            }
            .to_string(),
            "preset 3"
        );
        assert_eq!(
            ObjectRef::View {
                view_id: ViewId::new(3)
            }
            .to_string(),
            "view 3"
        );
        assert_eq!(
            ObjectRef::Executor {
                executor_id: ExecutorId::new(3)
            }
            .to_string(),
            "executor 3"
        );
        assert_eq!(
            ObjectRef::Group {
                group_id: GroupId::new(3)
            }
            .noun(),
            "group"
        );
    }

    /// **A cue that names no sequence carries a `null`, not an absence.**
    ///
    /// The distinction is worth a test because it is a decision: `#[ts(optional)]`
    /// would have rendered `sequenceId?: SequenceId`, which says *not supplied*,
    /// and this field says something more definite — *the cue list the session
    /// has selected* (`ARCHITECTURE_SPEC.md` §4.1). A `null` says that and a
    /// missing key does not, and a recording is easier to read for it.
    ///
    /// A file or a client from before S40 that leaves the key out is still
    /// accepted, because the field is `#[serde(default)]`.
    #[test]
    fn a_cue_that_names_no_sequence_says_so_with_a_null() {
        assert_eq!(
            serde_json::to_string(&ObjectRef::Cue {
                sequence_id: None,
                cue_number: "5".to_owned()
            })
            .unwrap(),
            r#"{"t":"Cue","sequenceId":null,"cueNumber":"5"}"#
        );
        let back: ObjectRef = serde_json::from_str(r#"{"t":"Cue","cueNumber":"5"}"#).unwrap();
        assert_eq!(
            back,
            ObjectRef::Cue {
                sequence_id: None,
                cue_number: "5".to_owned()
            }
        );
    }

    #[test]
    fn a_window_cannot_be_placed_at_a_coordinate_that_is_not_a_number() {
        // The same guard `WindowInstance` carries, on the way in as well as on
        // the way out: MessagePack can encode NaN and infinity faithfully, so
        // without this a hostile or corrupt frame could put one into the
        // session — where it would compare unequal to itself and stop the show
        // file saving.
        let placed = |x: f64| Command::PlaceWindow {
            instance_id: WindowInstanceId::new(1),
            x,
            y: 0.0,
            w: 1.0,
            h: 1.0,
        };
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(serde_json::to_string(&placed(value)).is_err());
            assert!(rmp_serde::to_vec_named(&placed(value)).is_err());
        }
        let json = serde_json::to_string(&placed(-12.5)).unwrap();
        assert_eq!(
            json,
            r#"{"t":"PlaceWindow","instanceId":1,"x":-12.5,"y":0.0,"w":1.0,"h":1.0}"#
        );
        assert_eq!(
            serde_json::from_str::<Command>(&json).unwrap(),
            placed(-12.5)
        );
        assert!(serde_json::from_str::<Command>(&json.replace("-12.5", "1e400")).is_err());
    }

    #[test]
    fn playback_and_session_commands_are_not_undoable() {
        // ARCHITECTURE_SPEC.md §6.1: Oops must not change light the operator is
        // currently driving, nor pull windows out from under them.
        for command in [
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
            // S34's, and it is the *most* playback-shaped of them: a button on
            // a strip during a show.
            Command::ExecutorButton {
                executor_id: ExecutorId::new(0),
                button: ExecutorButtonRef::Slot { index: 0 },
                pressed: true,
            },
            Command::SelectView {
                view_id: ViewId::new(1),
            },
            // S39's session command, excluded for every session command's
            // reason: an Oops must not pull a cue list out from under an
            // operator any more than it pulls a window.
            Command::SelectSequence {
                sequence_id: SequenceId::new(1),
            },
            Command::Oops,
            Command::Redo,
            Command::SaveShow,
        ] {
            assert!(!command.is_undoable(), "{command:?}");
        }

        for command in [
            Command::ClearProgrammer,
            Command::ApplyPreset {
                preset_id: PresetId::new(1),
            },
            Command::StoreCue {
                sequence_id: Some(SequenceId::new(1)),
                cue_number: "1".to_owned(),
                mode: StoreMode::Remove,
            },
            // S39's three, and `Update` is the one worth naming: it is a store,
            // so it is a show edit, so `ARCHITECTURE_SPEC.md` §6.1 makes it
            // undoable however playback-shaped the key on the desk looks.
            Command::StoreSequence {
                sequence_id: SequenceId::new(1),
                name: String::new(),
                mode: SequenceStoreMode::Override,
            },
            Command::EditCue {
                sequence_id: Some(SequenceId::new(1)),
                cue_number: "1".to_owned(),
            },
            Command::Update,
            // The three S27 added. A patch edit is exactly the kind of thing
            // Oops is for: it is not light the operator is currently driving.
            Command::UnpatchFixture {
                id: FixtureId::new(1),
            },
            Command::RenumberFixture {
                id: FixtureId::new(1),
                to: FixtureId::new(2),
            },
            Command::EmbedFixtureType {
                type_id: "generic.dimmer".to_owned(),
            },
        ] {
            assert!(command.is_undoable(), "{command:?}");
        }
    }

    #[test]
    fn selection_modes_and_directions_are_named() {
        assert_eq!(
            serde_json::to_string(&SelectionMode::Toggle).unwrap(),
            "\"Toggle\""
        );
        assert_eq!(
            serde_json::to_string(&GoDirection::Prev).unwrap(),
            "\"Prev\""
        );
        assert_eq!(
            serde_json::to_string(&ParamDirection::Next).unwrap(),
            "\"Next\""
        );
    }
}
