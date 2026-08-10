//! Deltas — `docs/IPC_PROTOCOL.md` §6.
//!
//! A delta describes a change that has **already been applied**. Clients apply
//! deltas to their mirror without validating them: the daemon has already
//! decided. A client that has applied every delta since its snapshot holds state
//! identical to the daemon's.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{ExecutorId, JsonPatchOp, OutputId, ProgrammerState, output::OutputHealth};

/// Severity of a [`Delta::Notice`], matching the logger levels in `CLAUDE.md`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize, TS,
)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum NoticeLevel {
    /// Informational.
    #[default]
    Info,
    /// Something the operator should know about but that did not stop the show.
    Warn,
    /// Something failed.
    Error,
}

/// A change the daemon has already applied.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(tag = "t", rename_all_fields = "camelCase")]
pub enum Delta {
    /// Patch, sequences, presets, groups.
    ShowPatch {
        /// The operations to apply, in order.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::small_vec(3)")
        )]
        ops: Vec<JsonPatchOp>,
    },
    /// Views, windows, pages, selection.
    SessionPatch {
        /// The operations to apply, in order.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::small_vec(3)")
        )]
        ops: Vec<JsonPatchOp>,
    },
    /// The programmer changed. Sent whole: it is small and sparse.
    ProgrammerChanged {
        /// The new programmer state.
        state: ProgrammerState,
    },
    /// An executor started, stopped or moved to another cue.
    ExecutorState {
        /// The executor concerned.
        executor_id: ExecutorId,
        /// Whether it is now running.
        is_active: bool,
        /// Index of the current cue, if one is active.
        cue_index: Option<u32>,
    },
    /// A DMX output changed health.
    OutputHealth {
        /// The output concerned.
        output_id: OutputId,
        /// Its new health.
        health: OutputHealth,
    },
    /// The show has unsaved changes, or no longer has. Drives the console's
    /// Save LED.
    DirtyFlag {
        /// Whether unsaved changes exist.
        unsaved_changes: bool,
    },
    /// A message for the operator.
    Notice {
        /// Severity.
        level: NoticeLevel,
        /// Human-readable text.
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use crate::{
        Delta, ExecutorId, JsonPatchOp, JsonValue, NoticeLevel, OutputHealth, OutputId,
        ProgrammerState,
    };

    #[test]
    fn deltas_are_internally_tagged_with_t() {
        let delta = Delta::ShowPatch {
            ops: vec![JsonPatchOp::Replace {
                path: "/fixtures/1/name".to_owned(),
                value: JsonValue::String("Front left".to_owned()),
            }],
        };
        assert_eq!(
            serde_json::to_string(&delta).unwrap(),
            r#"{"t":"ShowPatch","ops":[{"op":"replace","path":"/fixtures/1/name","value":"Front left"}]}"#
        );
    }

    #[test]
    fn executor_state_reports_activity_and_cue_position() {
        let delta = Delta::ExecutorState {
            executor_id: ExecutorId::new(3),
            is_active: true,
            cue_index: Some(2),
        };
        assert_eq!(
            serde_json::to_string(&delta).unwrap(),
            r#"{"t":"ExecutorState","executorId":3,"isActive":true,"cueIndex":2}"#
        );
    }

    #[test]
    fn output_health_is_reported_per_output() {
        let delta = Delta::OutputHealth {
            output_id: OutputId::new(1),
            health: OutputHealth::Degraded,
        };
        assert_eq!(
            serde_json::to_string(&delta).unwrap(),
            r#"{"t":"OutputHealth","outputId":1,"health":"Degraded"}"#
        );
    }

    #[test]
    fn the_dirty_flag_drives_the_console_save_led() {
        assert_eq!(
            serde_json::to_string(&Delta::DirtyFlag {
                unsaved_changes: true
            })
            .unwrap(),
            r#"{"t":"DirtyFlag","unsavedChanges":true}"#
        );
    }

    #[test]
    fn notices_carry_a_level_and_a_message() {
        let delta = Delta::Notice {
            level: NoticeLevel::Warn,
            message: "Output 1 disconnected".to_owned(),
        };
        assert_eq!(
            serde_json::to_string(&delta).unwrap(),
            r#"{"t":"Notice","level":"Warn","message":"Output 1 disconnected"}"#
        );
    }

    #[test]
    fn every_delta_from_the_protocol_specification_exists() {
        let deltas = [
            Delta::ShowPatch { ops: vec![] },
            Delta::SessionPatch { ops: vec![] },
            Delta::ProgrammerChanged {
                state: ProgrammerState::default(),
            },
            Delta::ExecutorState {
                executor_id: ExecutorId::new(0),
                is_active: false,
                cue_index: None,
            },
            Delta::OutputHealth {
                output_id: OutputId::new(0),
                health: OutputHealth::Ok,
            },
            Delta::DirtyFlag {
                unsaved_changes: false,
            },
            Delta::Notice {
                level: NoticeLevel::Info,
                message: String::new(),
            },
        ];
        assert_eq!(deltas.len(), 7);
        for delta in deltas {
            let json = serde_json::to_string(&delta).unwrap();
            let back: Delta = serde_json::from_str(&json).unwrap();
            assert_eq!(back, delta);
        }
    }

    #[test]
    fn notice_levels_match_the_logger_levels() {
        for (level, text) in [
            (NoticeLevel::Info, "\"Info\""),
            (NoticeLevel::Warn, "\"Warn\""),
            (NoticeLevel::Error, "\"Error\""),
        ] {
            assert_eq!(serde_json::to_string(&level).unwrap(), text);
        }
    }
}
