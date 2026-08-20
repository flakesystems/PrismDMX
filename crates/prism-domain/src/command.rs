//! Commands — `docs/IPC_PROTOCOL.md` §5.
//!
//! A command expresses intent. The daemon validates it, applies it, journals it
//! for Oops where applicable and broadcasts the resulting [`crate::Delta`]. A
//! command that cannot be applied changes nothing.
//!
//! The second half of the list is the concrete form of decision **D11**: the
//! console and the UI draw on one vocabulary, so there is no second command
//! world to keep in sync.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    AttributeType, CueProperty, ExecutorButtonRef, ExecutorId, FeatureGroup, FixtureId, JsonValue,
    PresetId, RgbColor, SequenceId, UniverseId, ViewId, WindowInstanceId, WindowType,
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
        mode: SelectionMode,
    },
    /// Set an attribute on the current selection.
    SetAttribute {
        /// The attribute to change.
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
    /// Apply a preset to the current selection.
    ApplyPreset {
        /// The preset to apply.
        preset_id: PresetId,
    },
    /// Advance the three-stage Clear.
    ClearProgrammer,
    /// Store the programmer contents into a cue.
    ///
    /// **Merged, not overwritten** — `prism_core::Programmer::cue`, whose
    /// documentation names Merge / Override / Remove as **S39**'s distinction to
    /// build. There is deliberately no mode on this command yet: a client that
    /// carried one the daemon did not honour would be describing an outcome that
    /// did not happen. What S28 does instead is *say so first* — `Query::
    /// StorePreview` answers with the mode and with what it would add, replace
    /// and keep, and the interface puts that on the button.
    StoreCue {
        /// Target sequence.
        sequence_id: SequenceId,
        /// Cue number as typed, e.g. `1.5`.
        cue_number: String,
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
        /// Which pool it is filed in, and which values it takes.
        pool: FeatureGroup,
        /// Operator-facing name.
        name: String,
        /// Colour for the scribble strip, if one was chosen.
        color: Option<RgbColor>,
    },
    /// Create an empty sequence.
    ///
    /// **Not `StoreSequence`**, which is S39's and is a different act: that one
    /// stores the *programmer* into a sequence with a mode. This one makes the
    /// cue list exist, which is what [`Self::StoreCue`] needs before it can put a
    /// cue anywhere and what a show with nothing in it has none of. It is refused
    /// when the number is taken, so it can never empty a cue list that is on
    /// stage.
    CreateSequence {
        /// The sequence number.
        sequence_id: SequenceId,
        /// Operator-facing name.
        name: String,
    },
    /// Change one field of one cue.
    ///
    /// One field rather than a whole cue — see [`CueProperty`]. What a cue
    /// *does* is not among the fields: values come from the programmer through
    /// [`Self::StoreCue`], the same rule that keeps channels out of
    /// [`Self::PatchFixture`].
    SetCueProperty {
        /// The sequence the cue is in.
        sequence_id: SequenceId,
        /// The cue, by the number it has **now**.
        cue_number: String,
        /// The field, and its new value.
        property: CueProperty,
    },
    /// Take a cue out of a sequence.
    ///
    /// Does not cascade and does not renumber: the cues after it keep their
    /// numbers, because a cue number is what an operator has written on a running
    /// order and what an F-key may be bound to.
    DeleteCue {
        /// The sequence.
        sequence_id: SequenceId,
        /// The cue, by number.
        cue_number: String,
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
    /// Step an executor.
    ExecutorGo {
        /// Target executor.
        executor_id: ExecutorId,
        /// Which way to step.
        direction: GoDirection,
    },
    /// Stop an executor.
    ExecutorOff {
        /// Target executor.
        executor_id: ExecutorId,
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
    /// Rename a stored view, leaving its windows alone.
    ///
    /// **Not one of `ARCHITECTURE_SPEC.md` §4.4's eleven**, for the same reason
    /// as [`Self::PlaceWindow`]: §4.4 lists what the *console* issues, and an
    /// X-Touch has no way to type a name. It travels with the twelve, is
    /// journalled with them — that is, not at all, §6.1 — and exists because
    /// §4.1 puts the view library in the session, so a client that renamed a
    /// view locally would be holding session state.
    ///
    /// Deliberately separate from [`Self::StoreView`], which overwrites the
    /// windows: renaming a view must not silently replace the layout in it with
    /// whatever happens to be on the canvas.
    RenameView {
        /// The view to rename.
        view_id: ViewId,
        /// The new name.
        name: String,
    },
    /// Delete a stored view.
    ///
    /// The **last** view cannot be deleted: `activeViewId` names a view from the
    /// first moment (`prism_core::SessionState::new`) and a session whose active
    /// view is not a view is a dangling reference. Deleting the *active* view is
    /// allowed, and what the canvas then shows is the daemon's to decide — see
    /// `prism_core::SessionState::delete_view`.
    DeleteView {
        /// The view to delete.
        view_id: ViewId,
    },
    /// Move a stored view one place along the bar.
    ///
    /// # The number is the order
    ///
    /// Views are held by number and drawn in number order, and `Channel ◀▶`
    /// (**D8**) steps that same order. So moving a view **exchanges its number
    /// with its neighbour's** rather than recording an order beside the numbers:
    /// there is one order, and the console cannot disagree with the screen about
    /// what comes next because there is nothing for it to disagree with.
    ///
    /// The cost is real and deliberate: after a move, `SelectView 3` names a
    /// different layout, and an F-key bound to a view number follows the *place*
    /// rather than the layout that used to be there. That is how a console's
    /// page numbers behave, and it is the price of the two never drifting apart.
    MoveView {
        /// The view to move.
        view_id: ViewId,
        /// Which way along the bar.
        direction: ParamDirection,
    },
    /// Open a window on the canvas.
    OpenWindow {
        /// Which window to open.
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
    /// Switch the encoder bank.
    SetEncoderBank {
        /// The feature group to switch to.
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
        direction: ParamDirection,
    },
    /// Type into the command line.
    CommandLineInput {
        /// The text entered.
        text: String,
    },
}

impl Command {
    /// Whether this command acts on session state rather than on the show.
    ///
    /// `ARCHITECTURE_SPEC.md` §4.4's eleven, plus [`Self::PlaceWindow`], which
    /// is twelfth because §4.4 lists what the *console* issues and a canvas is
    /// not a console, plus [`Self::RenameView`], [`Self::DeleteView`] and
    /// [`Self::MoveView`], which are thirteenth to fifteenth for the same
    /// reason: managing a view library is something an operator does with a
    /// pointer, and §4.1 puts that library in the session. See those variants
    /// for why they have to exist at all.
    #[must_use]
    pub const fn is_session_command(&self) -> bool {
        matches!(
            self,
            Self::SelectView { .. }
                | Self::StoreView { .. }
                | Self::RenameView { .. }
                | Self::DeleteView { .. }
                | Self::MoveView { .. }
                | Self::OpenWindow { .. }
                | Self::CloseWindow { .. }
                | Self::FocusWindow { .. }
                | Self::PlaceWindow { .. }
                | Self::SetExecutorPage { .. }
                | Self::SelectExecutor { .. }
                | Self::SetEncoderBank { .. }
                | Self::SetProgrammerPage { .. }
                | Self::SelectProgrammerParam { .. }
                | Self::CommandLineInput { .. }
        )
    }

    /// Whether applying this command should push an entry onto the Oops journal.
    ///
    /// `ARCHITECTURE_SPEC.md` §6.1: playback actions and every session command
    /// are deliberately excluded, so undo during a running show neither changes
    /// light the operator is driving nor pulls windows out from under them.
    /// `Oops`, `Redo` and `SaveShow` are excluded because they are not show
    /// mutations in the first place.
    #[must_use]
    pub const fn is_undoable(&self) -> bool {
        !matches!(
            self,
            Self::ExecutorGo { .. }
                | Self::ExecutorOff { .. }
                | Self::ExecutorButton { .. }
                | Self::SetExecutorMaster { .. }
                | Self::Oops
                | Self::Redo
                | Self::SaveShow
        ) && !self.is_session_command()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        AttributeType, Command, CueProperty, ExecutorButtonRef, ExecutorId, FeatureGroup,
        FixtureId, GoDirection, JsonValue, ParamDirection, PresetId, RgbColor, SelectionMode,
        SequenceId, UniverseId, ViewId, WindowInstanceId, WindowType,
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
                pool: FeatureGroup::Color,
                name: "Deep blue".to_owned(),
                color: Some(RgbColor { r: 0, g: 0, b: 255 }),
            },
            Command::CreateSequence {
                sequence_id: SequenceId::new(1),
                name: "Act 1".to_owned(),
            },
            Command::SetCueProperty {
                sequence_id: SequenceId::new(1),
                cue_number: "1".to_owned(),
                property: CueProperty::Name {
                    name: "Blackout".to_owned(),
                },
            },
            Command::DeleteCue {
                sequence_id: SequenceId::new(1),
                cue_number: "1".to_owned(),
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
            Command::RenameView {
                view_id: ViewId::new(1),
                name: "Busking".to_owned(),
            },
            Command::DeleteView {
                view_id: ViewId::new(1),
            },
            Command::MoveView {
                view_id: ViewId::new(1),
                direction: ParamDirection::Next,
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
        ];
        assert_eq!(commands.len(), 36);

        // Every command must survive the wire, and the tag must be stable.
        for command in commands {
            let json = serde_json::to_string(&command).unwrap();
            assert!(json.starts_with(r#"{"t":""#), "{json}");
            let back: Command = serde_json::from_str(&json).unwrap();
            assert_eq!(back, command);
        }
    }

    #[test]
    fn session_commands_are_the_architecture_spec_list_plus_the_ones_a_screen_needs() {
        // ARCHITECTURE_SPEC.md §4.4's eleven, plus the four a *screen* needs and
        // a console cannot issue: `PlaceWindow`, because §4.1 puts a window's
        // position and size in the session and an X-Touch never drags one, and
        // `RenameView` / `DeleteView` / `MoveView`, because §4.1 puts the view
        // library there too and an X-Touch cannot type a name. Each of them is a
        // command or it is client-local state pretending not to be.
        let session_commands = [
            Command::SelectView {
                view_id: ViewId::new(1),
            },
            Command::StoreView {
                view_id: ViewId::new(1),
                name: String::new(),
            },
            Command::RenameView {
                view_id: ViewId::new(1),
                name: String::new(),
            },
            Command::DeleteView {
                view_id: ViewId::new(1),
            },
            Command::MoveView {
                view_id: ViewId::new(1),
                direction: ParamDirection::Next,
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
        ];
        assert_eq!(session_commands.len(), 15);
        for command in session_commands {
            assert!(command.is_session_command(), "{command:?}");
        }
        assert!(!Command::ClearProgrammer.is_session_command());
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
                sequence_id: SequenceId::new(1),
                cue_number: "1".to_owned(),
            },
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
