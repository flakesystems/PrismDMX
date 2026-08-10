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
    AttributeType, ExecutorId, FeatureGroup, FixtureId, JsonValue, PresetId, SequenceId,
    UniverseId, ViewId, WindowInstanceId, WindowType,
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
    StoreCue {
        /// Target sequence.
        sequence_id: SequenceId,
        /// Cue number as typed, e.g. `1.5`.
        cue_number: String,
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
    /// Move an executor's master.
    SetExecutorMaster {
        /// Target executor.
        executor_id: ExecutorId,
        /// New level, `0..=65535`.
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
    /// Whether this command is one of the interface commands from
    /// `ARCHITECTURE_SPEC.md` §4.4, which act on session state.
    #[must_use]
    pub const fn is_session_command(&self) -> bool {
        matches!(
            self,
            Self::SelectView { .. }
                | Self::StoreView { .. }
                | Self::OpenWindow { .. }
                | Self::CloseWindow { .. }
                | Self::FocusWindow { .. }
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
        AttributeType, Command, ExecutorId, FeatureGroup, FixtureId, GoDirection, JsonValue,
        ParamDirection, PresetId, SelectionMode, SequenceId, UniverseId, ViewId, WindowInstanceId,
        WindowType,
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
        assert_eq!(commands.len(), 23);

        // Every command must survive the wire, and the tag must be stable.
        for command in commands {
            let json = serde_json::to_string(&command).unwrap();
            assert!(json.starts_with(r#"{"t":""#), "{json}");
            let back: Command = serde_json::from_str(&json).unwrap();
            assert_eq!(back, command);
        }
    }

    #[test]
    fn session_commands_are_exactly_the_list_in_the_architecture_spec() {
        // ARCHITECTURE_SPEC.md §4.4.
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
        assert_eq!(session_commands.len(), 11);
        for command in session_commands {
            assert!(command.is_session_command(), "{command:?}");
        }
        assert!(!Command::ClearProgrammer.is_session_command());
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
