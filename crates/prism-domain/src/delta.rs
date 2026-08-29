//! Deltas — `docs/IPC_PROTOCOL.md` §6.
//!
//! A delta describes a change that has **already been applied**. Clients apply
//! deltas to their mirror without validating them: the daemon has already
//! decided. A client that has applied every delta since its snapshot holds state
//! identical to the daemon's.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    JsonPatchOp, MachineSettings, OutputId, OutputInstance, PlaybackId, ProgrammerState,
    ShowFileInfo, output::OutputHealth,
};

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
    /// A playback started, stopped or moved to another cue.
    ///
    /// **It was `ExecutorState` until S40**, and the rename is the whole of that
    /// session's playback change in one line: a cue list on no fader can now
    /// play, so the thing that reports is a [`crate::PlaybackId`] rather than an
    /// executor number. A strip still gets exactly what it got before, because
    /// `PlaybackId::Executor` is what an executor reports as.
    ///
    /// The protocol gives running playbacks their own delta rather than folding
    /// them into a `ShowPatch` so a client does not have to diff the show to
    /// draw a moving executor bar.
    PlaybackState {
        /// Which playback.
        playback: PlaybackId,
        /// Whether it is now running.
        is_active: bool,
        /// Index of the current cue, if one is active.
        cue_index: Option<u32>,
    },
    /// **This machine's** output patch changed — S33.
    ///
    /// Sent whole, like [`Self::ProgrammerChanged`] and for the same reason: it
    /// is small and sparse, a rig is a handful of rows rather than a document,
    /// and a client that had to diff a JSON patch to redraw five status lights
    /// would be doing arithmetic to learn something it can simply be told.
    ///
    /// It is deliberately **not** a `ShowPatch`. The output patch is not show
    /// content — see `Command::is_machine_command` — so it must not travel in
    /// the document a client mirrors as the show, or a client would write the
    /// venue's cabling into its idea of the show and a save would be next.
    OutputsChanged {
        /// The whole rig as it now stands, in output-number order.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::small_vec(2)")
        )]
        outputs: Vec<OutputInstance>,
    },
    /// **This machine's** control surface is on a different MIDI port — S36.
    ///
    /// The one-field counterpart of [`Self::OutputsChanged`], and it is a delta
    /// for the same reason: the configured port belongs to the machine rather
    /// than to the show, so it must not travel inside the document a client
    /// mirrors as the show. Sent whole because it *is* whole — one name, or none.
    ///
    /// What is **not** here is the list of ports that exist. That is not state
    /// the daemon owns: it changes when a person moves a plug, no command causes
    /// it, and a client holding a copy would be holding the operating system's
    /// opinion from whenever it last connected. It is asked for —
    /// [`crate::Query::MidiPorts`] — and this delta is what tells a settings
    /// window that asking again is worth it.
    SurfaceChanged {
        /// The configured port, or `None` for no surface at all.
        port: Option<String>,
    },
    /// The binding table moved — S38.
    ///
    /// [`Self::SurfaceChanged`]'s shape for what the desk's keys *do*, and it
    /// carries a change token rather than the table for that delta's reason
    /// exactly: the table is seventy-three rows that only an open control editor
    /// is looking at, and it is **derived** (§4.1's defaults, a profile file, and
    /// this machine's own rows), so it is asked for —
    /// [`crate::Query::SurfaceBindings`] — and this is what says asking again is
    /// worth it.
    ///
    /// The revision is what makes *two clients, one table* something a test can
    /// assert rather than something a design hopes for: both editors are told the
    /// same number, and an answer that names an older one has been overtaken.
    SurfaceBindingsChanged {
        /// How many times the table has moved since the daemon started.
        revision: u32,
    },
    /// Learn was armed, gave up, or has just named a control — S38.
    ///
    /// Broadcast rather than answered to the one client that asked, and that is
    /// the decision in this variant: **there is one desk**, so there is one
    /// learn. Two editors open on two screens must not both believe they have
    /// armed it, and the operator standing at the console pressing a key has no
    /// idea which browser asked. It is the same argument `ARCHITECTURE_SPEC.md`
    /// §4 makes for the active view, applied to a mode instead of a layout.
    ///
    /// The two moments are one variant because they are one fact — *what is
    /// learn doing* — read at two times: arming is `{ learning: true, control:
    /// None }`, and a control being named is `{ learning: false, control:
    /// Some(_) }`, because learn is one shot.
    SurfaceLearnChanged {
        /// Whether learn is armed **after** this event.
        learning: bool,
        /// The control the operator just touched, if this is the moment one was
        /// named.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        control: Option<crate::BoundControl>,
    },
    /// One of **this machine's** settings changed — S37.
    ///
    /// [`Self::OutputsChanged`]'s shape for the rest of the machine, and the
    /// same three reasons: it is small, it is not a document, and a client that
    /// had to diff a JSON patch to redraw a settings panel would be doing
    /// arithmetic to learn something it can be told. Both mirrors ignore it by
    /// name, because what this desk is set to is no more show content than its
    /// cabling is.
    ///
    /// It carries the settings **whole**, including the two things that are not
    /// configuration at all — where the daemon's data directory is, and which
    /// settings this run's command line is holding. Those change when a command
    /// changes them and at no other time, which is the test a delta has to pass.
    MachineChanged {
        /// Every field of the *This machine* panel.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        settings: MachineSettings,
    },
    /// The daemon opened, made or renamed a show file — S37.
    ///
    /// Which `.prism` file is open is state the daemon owns and that only a
    /// command changes, so it is a delta rather than a question. It carries the
    /// recent list with it because the two move together: a show that has just
    /// been opened is the one that has just left the list.
    ///
    /// It is **not** the Save lamp, which is [`Self::DirtyFlag`] and moves far
    /// more often; the flag is repeated inside for the panel's convenience and
    /// the lamp is still the thing the console reads.
    ShowFileChanged {
        /// The file, the recent ones, and the autosave's state.
        #[cfg_attr(
            any(test, feature = "proptest"),
            proptest(strategy = "crate::arb::boxed()")
        )]
        file: ShowFileInfo,
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
        Delta, JsonPatchOp, JsonValue, NoticeLevel, OutputHealth, OutputId, OutputInstance,
        OutputKind, PlaybackId, ProgrammerState, SequenceId, UniverseId,
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

    /// S45: a playback is a cue list's, so the delta names one number.
    ///
    /// It was a tagged object with an executor in one arm until then, and the
    /// executors are exactly what B18 found two of. What a client draws on an
    /// executor row it now reads through the list standing on it.
    #[test]
    fn playback_state_reports_activity_and_cue_position_for_a_cue_list() {
        let delta = Delta::PlaybackState {
            playback: PlaybackId::of_sequence(SequenceId::new(3)),
            is_active: true,
            cue_index: Some(2),
        };
        assert_eq!(
            serde_json::to_string(&delta).unwrap(),
            r#"{"t":"PlaybackState","playback":3,"isActive":true,"cueIndex":null}"#
                .replace("null", "2")
        );
        let stopped = Delta::PlaybackState {
            playback: PlaybackId::of_sequence(SequenceId::new(7)),
            is_active: false,
            cue_index: None,
        };
        assert_eq!(
            serde_json::to_string(&stopped).unwrap(),
            r#"{"t":"PlaybackState","playback":7,"isActive":false,"cueIndex":null}"#
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

    /// S33: the rig travels whole, and it says which universes each output
    /// carries — which is the routing an operator reads off a settings panel.
    #[test]
    fn the_output_patch_travels_whole() {
        let delta = Delta::OutputsChanged {
            outputs: vec![OutputInstance::new(
                OutputId::new(1),
                "Hall",
                OutputKind::OpenDmx { serial: None },
                [UniverseId::new(1)],
            )],
        };
        let json = serde_json::to_string(&delta).unwrap();
        assert!(json.starts_with(r#"{"t":"OutputsChanged""#), "{json}");
        assert!(json.contains(r#""universes":[1]"#), "{json}");
        assert_eq!(serde_json::from_str::<Delta>(&json).unwrap(), delta);
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
            Delta::PlaybackState {
                playback: PlaybackId::of_sequence(SequenceId::new(0)),
                is_active: false,
                cue_index: None,
            },
            Delta::OutputsChanged {
                outputs: Vec::new(),
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
        assert_eq!(deltas.len(), 8);
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
