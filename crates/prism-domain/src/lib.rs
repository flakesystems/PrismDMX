//! Shared domain vocabulary for PrismDMX.
//!
//! Every other crate speaks these types. They are defined once here and exported
//! to TypeScript so the UI cannot drift from the daemon.
//!
//! See `ARCHITECTURE_SPEC.md` section 6 for the model and `docs/IPC_PROTOCOL.md`
//! sections 5-6 for the `Command` and `Delta` wire types.
//!
//! Implemented in session **S1**. This crate is platform-neutral: it must
//! compile on every target, so it may not contain `#[cfg(target_os = ...)]`.
//!
//! # Wire format
//!
//! Every type derives `serde` with `camelCase` field names, and the two wire
//! enums are internally tagged with `t`, exactly as `docs/IPC_PROTOCOL.md`
//! writes them. Internal tagging means `prism-ipc` must serialise MessagePack
//! with **named** fields (`rmp_serde::to_vec_named`); the default array encoding
//! cannot carry a tag. The round-trip tests in `wire.rs` assert both encodings.
//!
//! # TypeScript
//!
//! [`export_bindings`] writes `ui/src/bindings/` and is run by this crate's test
//! suite, so `cargo test -p prism-domain` regenerates the bindings and a stale
//! binding cannot survive a green build.

#[cfg(any(test, feature = "proptest"))]
pub mod arb;

mod attribute;
mod command;
mod delta;
mod executor;
mod export;
pub mod finite;
mod ids;
mod json;
mod machine;
mod midi;
mod output;
mod patch;
mod playback;
mod preset;
mod programmer;
mod query;
mod sequence;
mod session;
pub mod socket;
mod surface;
#[cfg(test)]
mod wire;

pub use attribute::{AttributeDef, AttributeType, FeatureGroup, FixtureType, MergeMode};
pub use command::{
    Command, GoDirection, ObjectRef, OutputChange, OverwriteMode, ParamDirection, SelectionMode,
    SequenceStoreMode,
};
pub use delta::{Delta, NoticeLevel};
pub use executor::{
    EXECUTORS_PER_PAGE, Executor, ExecutorButtonFunction, ExecutorButtonRef,
    ExecutorEncoderFunction, ExecutorFaderFunction, SPEED_UNITY,
};
pub use export::{BINDINGS_DIR, export_bindings};
pub use ids::{
    ExecutorId, FixtureId, GroupId, OutputId, PresetId, SequenceId, SessionId, UniverseId, ViewId,
    WindowInstanceId,
};
pub use json::{JsonPatchOp, JsonValue};
pub use machine::{
    ExitAction, LogLevel, MachineChange, MachineOverride, MachineSettings, ShowFileInfo,
};
pub use midi::{MidiPortInfo, SurfaceHealth, SurfaceStatus};
pub use output::{
    ArtNetPort, OutputHealth, OutputInstance, OutputKind, OutputStatusInfo, SacnPort,
};
pub use patch::{CHANNELS_PER_UNIVERSE, Fixture, Group, RgbColor, Vec3};
pub use playback::{PlaybackId, PlaybackTarget};
pub use preset::{Preset, PresetPool, PresetValue};
pub use programmer::{
    ClearStage, InvalidClearStage, ProgrammerEntry, ProgrammerState, ProgrammerValue,
    ProgrammerValueSource, ProgrammerValues,
};
pub use query::{
    Answer, LibraryEntry, PatchConflict, PatchPreview, Query, StoreMode, StorePreview, StoreTarget,
};
pub use sequence::{Cue, CueEdit, CuePart, CueProperty, CueTrigger, Sequence};
pub use session::{
    CANVAS_HEIGHT, CANVAS_WIDTH, MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH, Session, View,
    WindowInstance, WindowType,
};
pub use surface::{
    BoundControl, ExecutorTarget, GlobalButton, RESERVED_BUTTONS, RESERVED_REASON, Step,
    StripButton, SurfaceAction, SurfaceBinding, SurfaceControl,
};
