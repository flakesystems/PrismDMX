//! Round-trip property tests for every domain type.
//!
//! S1 exit criterion: *every type survives serialise → deserialise unchanged*.
//! Both codecs are checked, because both are real:
//!
//! - **JSON** — show export/import (`IMPLEMENTATION_PLAN.md` S15) and every
//!   human-readable dump.
//! - **MessagePack** — the IPC wire (`docs/IPC_PROTOCOL.md` §3). It is encoded
//!   with `to_vec_named`, because the compact array encoding cannot carry the
//!   `t` tag of an internally tagged enum.
//!
//! The last test in this module closes the loop: it asserts that every type with
//! a generated TypeScript binding also appears in the list below, so a new type
//! cannot be added and quietly skip its round-trip.

use proptest::prelude::*;

/// One round-trip property per type, plus the list of covered type names.
macro_rules! round_trip {
    ($($name:ident => $ty:ty),* $(,)?) => {
        /// Every type covered by a round-trip property, by TypeScript name.
        const COVERED: &[&str] = &[$(stringify!($ty)),*];

        $(
            proptest! {
                #![proptest_config(ProptestConfig::with_cases(64))]

                #[test]
                fn $name(value: $ty) {
                    let json = serde_json::to_string(&value)
                        .expect("every domain type must serialise to JSON");
                    let from_json: $ty = serde_json::from_str(&json)
                        .expect("JSON produced by this type must deserialise");
                    prop_assert_eq!(&value, &from_json, "JSON round trip changed the value");

                    let packed = rmp_serde::to_vec_named(&value)
                        .expect("every domain type must serialise to MessagePack");
                    let from_msgpack: $ty = rmp_serde::from_slice(&packed)
                        .expect("MessagePack produced by this type must deserialise");
                    prop_assert_eq!(&value, &from_msgpack, "MessagePack round trip changed the value");
                }
            }
        )*
    };
}

round_trip! {
    fixture_id => crate::FixtureId,
    group_id => crate::GroupId,
    sequence_id => crate::SequenceId,
    executor_id => crate::ExecutorId,
    preset_id => crate::PresetId,
    universe_id => crate::UniverseId,
    session_id => crate::SessionId,
    view_id => crate::ViewId,
    window_instance_id => crate::WindowInstanceId,
    output_id => crate::OutputId,

    attribute_type => crate::AttributeType,
    feature_group => crate::FeatureGroup,
    merge_mode => crate::MergeMode,
    attribute_def => crate::AttributeDef,
    fixture_type => crate::FixtureType,

    vec3 => crate::Vec3,
    rgb_color => crate::RgbColor,
    fixture => crate::Fixture,
    group => crate::Group,

    preset_value => crate::PresetValue,
    preset => crate::Preset,

    cue_trigger => crate::CueTrigger,
    cue_part => crate::CuePart,
    cue => crate::Cue,
    sequence => crate::Sequence,

    executor_button_function => crate::ExecutorButtonFunction,
    executor_fader_function => crate::ExecutorFaderFunction,
    executor_encoder_function => crate::ExecutorEncoderFunction,
    executor => crate::Executor,

    programmer_value_source => crate::ProgrammerValueSource,
    clear_stage => crate::ClearStage,
    programmer_value => crate::ProgrammerValue,
    programmer_entry => crate::ProgrammerEntry,
    programmer_state => crate::ProgrammerState,

    window_type => crate::WindowType,
    window_instance => crate::WindowInstance,
    view => crate::View,
    session => crate::Session,

    json_value => crate::JsonValue,
    json_patch_op => crate::JsonPatchOp,
    output_health => crate::OutputHealth,
    notice_level => crate::NoticeLevel,

    selection_mode => crate::SelectionMode,
    go_direction => crate::GoDirection,
    param_direction => crate::ParamDirection,
    command => crate::Command,
    delta => crate::Delta,

    library_entry => crate::LibraryEntry,
    patch_conflict => crate::PatchConflict,
    patch_preview => crate::PatchPreview,
    query => crate::Query,
    answer => crate::Answer,
}

/// Pins the reason [`crate::arb::finite_f64`] generates bounded-precision
/// values.
///
/// `serde_json` writes the shortest text that identifies an `f64` — the standard
/// library reads that text back exactly — but `serde_json`'s own parser is not
/// correctly rounded and returns a neighbouring value. MessagePack, being binary
/// IEEE 754, is exact.
///
/// This matters beyond the tests: the show export in S15 is JSON, so a saved and
/// reloaded show must not be assumed bit-identical in its floats unless values
/// are written at a bounded precision.
#[test]
fn json_floats_are_only_exact_to_bounded_precision() {
    let full_precision = -971_744.715_983_591_5_f64;
    let text = serde_json::to_string(&full_precision).unwrap();
    assert_eq!(text.parse::<f64>().unwrap(), full_precision);
    assert_ne!(
        serde_json::from_str::<f64>(&text).unwrap(),
        full_precision,
        "serde_json's float parser has become exact - the bounded-precision \
         strategies in `arb` can be widened"
    );

    let bounded = -971_744.716_f64;
    assert_eq!(
        serde_json::from_str::<f64>(&serde_json::to_string(&bounded).unwrap()).unwrap(),
        bounded
    );
}

/// The TypeScript name of a covered type, i.e. `Command` from `crate :: Command`
/// as `stringify!` renders it.
fn short_name(path: &str) -> &str {
    path.rsplit(':').next().unwrap_or(path).trim()
}

#[test]
fn every_exported_type_has_a_round_trip_property() {
    let covered: Vec<&str> = COVERED.iter().copied().map(short_name).collect();
    let missing: Vec<&String> = crate::export::exported_once()
        .iter()
        .filter(|name| !covered.contains(&name.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "these types have TypeScript bindings but no round-trip property: {missing:?}"
    );
}
