//! Strategies for property tests.
//!
//! Available to this crate's own tests and, behind the `proptest` feature, to
//! downstream crates: `prism-engine` verifies the merge invariants in
//! `docs/DMX_MERGE.md` over arbitrary domain values, and generating those values
//! is a property of the domain, not of the engine.
//!
//! Three constraints shape everything here:
//!
//! - **Finite floats only.** JSON has no encoding for NaN or infinity, and
//!   `NaN != NaN` would break every round-trip assertion for reasons that have
//!   nothing to do with the code under test.
//! - **Bounded decimal precision.** `serde_json`'s float *parser* is not
//!   correctly rounded: it re-reads its own output of a full-precision `f64` one
//!   ULP off. See `wire.rs::json_floats_are_only_exact_to_bounded_precision`,
//!   which pins that behaviour. Millithousandths of a degree, a metre or a
//!   second are far finer than anything a lighting desk resolves, so the
//!   strategies here generate values at that precision and the round-trip
//!   property stays an exact equality rather than a tolerance.
//! - **Small collections.** The model nests (sequence → cue → part), so the
//!   default size range of up to 100 elements per level would produce cases
//!   large enough to make shrinking useless.

use proptest::prelude::*;
use proptest::strategy::BoxedStrategy;

use crate::Command as C;
use crate::{
    AttributeType, CueProperty, ExecutorButtonRef, ExecutorId, FeatureGroup, FixtureId,
    GoDirection, GroupId, MachineChange, ObjectRef, OutputChange, OutputId, OutputInstance,
    OverwriteMode, ParamDirection, PlaybackTarget, PresetId, PresetPool, RgbColor, SelectionMode,
    SequenceId, SequenceStoreMode, StoreMode, UniverseId, ViewId, WindowInstanceId, WindowType,
};

/// Decimal places generated floats are limited to.
const SCALE: f64 = 1000.0;

/// A finite `f64` in a range wide enough for coordinates, angles and pixels.
pub fn finite_f64() -> impl Strategy<Value = f64> {
    (-1_000_000_000i64..1_000_000_000i64).prop_map(|units| units as f64 / SCALE)
}

/// A finite, non-negative `f64` for durations in seconds.
pub fn seconds() -> impl Strategy<Value = f64> {
    (0i64..600_000i64).prop_map(|units| units as f64 / SCALE)
}

/// A vector of at most `max` arbitrary values.
pub fn small_vec<T>(max: usize) -> impl Strategy<Value = Vec<T>>
where
    T: Arbitrary + 'static,
{
    proptest::collection::vec(any::<T>(), 0..max)
}

/// A map of at most `max` arbitrary entries.
pub fn small_map<K, V>(max: usize) -> impl Strategy<Value = std::collections::BTreeMap<K, V>>
where
    K: Arbitrary + Ord + 'static,
    V: Arbitrary + 'static,
{
    proptest::collection::btree_map(any::<K>(), any::<V>(), 0..max)
}

/// An arbitrary value, **behind a box**.
///
/// # This is a stack budget, not a style
///
/// `proptest_derive` builds one strategy value holding every variant's strategy
/// at once, so an enum's strategy type is the *sum* of its variants'. `Command`
/// is forty-two variants deep and in a debug build on Windows the sum has now
/// twice been enough to overflow a test thread's stack while the strategy is
/// still being constructed — S34's finding at the thirty-sixth variant, and
/// S40's again with [`crate::ObjectRef`] appearing twice inside `Copy` and twice
/// inside `Move`.
///
/// Boxing a *field's* strategy replaces its whole tree with one pointer, which
/// is why this is applied to the `ObjectRef` and `PlaybackTarget` fields rather
/// than to the enum as a whole. **A session adding a command with a large
/// payload reaches for this and not for `RUST_MIN_STACK`**, which only moves the
/// cliff a few variants further along.
pub fn boxed<T>() -> BoxedStrategy<T>
where
    T: Arbitrary + core::fmt::Debug + 'static,
{
    any::<T>().boxed()
}

/// A unit enum's strategy as an **index into its own list**, rather than as an
/// N-way union.
///
/// # This is a stack budget, and it is a bigger one than [`boxed`] can reach
///
/// `proptest_derive` builds one value tree with a slot for **every** variant,
/// and S38 measured that slot at **560 bytes whatever the variant carries** — a
/// bare unit variant costs the same as one with a payload. That makes a wide
/// enum of *nothing* the most expensive thing in the crate: `GlobalButton` is
/// sixty-four buttons carrying no data and its derived tree was **36 064
/// bytes**, larger than the whole of [`crate::Command`].
///
/// Everything holding one inherits it, and [`boxed`] cannot help: boxing a field
/// replaces that field's subtree with a pointer, but the subtree is still
/// **built** — `new_tree` constructs it as a local before the box takes it — so a
/// 36 KB tree behind a pointer is still a 36 KB frame at generation time. What
/// reaches it is not having the tree.
///
/// It is also the **better generator**, which is worth saying so it does not read
/// as a workaround: the derive weights variants equally by construction and so
/// does this, and a variant added to the enum joins the strategy by joining
/// `ALL` rather than by being remembered.
///
/// **Applied to every unit enum wide enough to matter**, so the budget in
/// `crate::wire` stops being a cliff a session can walk off by adding a name.
macro_rules! arbitrary_from_list {
    ($ty:ty) => {
        #[cfg(any(test, feature = "proptest"))]
        impl proptest::arbitrary::Arbitrary for $ty {
            type Parameters = ();
            type Strategy = proptest::strategy::BoxedStrategy<Self>;

            fn arbitrary_with((): Self::Parameters) -> Self::Strategy {
                use proptest::strategy::Strategy as _;
                (0..<$ty>::ALL.len())
                    .prop_map(|index| <$ty>::ALL[index])
                    .boxed()
            }
        }
    };
}

pub(crate) use arbitrary_from_list;

/// One of the surface's controls, as an index into [`crate::BoundControl::all`].
///
/// [`arbitrary_from_list`]'s trick for a type that is **not** a unit enum: the
/// list is not the variants, it is the seventy-three controls a desk has, and
/// indexing it is a `u32` tree rather than a nest of unions. Coverage is
/// unchanged — every control is reachable — and `crate::wire`'s
/// `bound_control` property still walks the type's own full space.
pub fn a_control() -> BoxedStrategy<crate::BoundControl> {
    let controls = crate::BoundControl::all();
    (0..controls.len())
        .prop_map(move |index| controls[index])
        .boxed()
}

/// One thing a control can be made to do, or nothing.
///
/// **Narrowed on purpose, and this is the trade.** `SurfaceAction` is seventeen
/// variants and its derived value tree is 16 176 bytes; built inside
/// `Command::ConfigureMachine`'s subtree it is enough to tip `prism-core`'s
/// `tests/oops.rs` over a debug test thread's stack — S38 measured exactly that,
/// twice. This list carries **every variant** with one representative payload,
/// so what is narrowed is the *combinations* rather than the vocabulary, and
/// `crate::wire`'s `surface_action` property still walks the type's own full
/// space with nothing else on the frame.
pub fn an_action() -> BoxedStrategy<Option<crate::SurfaceAction>> {
    use crate::SurfaceAction as A;
    use crate::{ExecutorTarget, Step};
    const ACTIONS: [Option<A>; 19] = [
        None,
        Some(A::ExecutorMaster {
            target: ExecutorTarget::Strip,
        }),
        Some(A::ExecutorGo {
            target: ExecutorTarget::Selected,
            direction: GoDirection::Next,
        }),
        Some(A::ExecutorOff {
            target: ExecutorTarget::Strip,
        }),
        Some(A::ExecutorButton {
            target: ExecutorTarget::Strip,
            button: ExecutorButtonRef::Slot { index: 0 },
        }),
        Some(A::SelectExecutor {
            target: ExecutorTarget::Strip,
        }),
        Some(A::ClearProgrammer),
        Some(A::ExecutorPage { delta: -1 }),
        Some(A::SelectView {
            view: ViewId::new(1),
        }),
        Some(A::StepView {
            direction: Step::Next,
        }),
        Some(A::ProgrammerPage { delta: 1 }),
        Some(A::SelectProgrammerParam {
            direction: ParamDirection::Prev,
        }),
        Some(A::AdjustParameter),
        Some(A::SetEncoderBank {
            group: FeatureGroup::Color,
        }),
        Some(A::OpenWindow {
            window: WindowType::Patch,
        }),
        Some(A::OpenWindowPicker),
        Some(A::SaveShow),
        Some(A::Oops),
        Some(A::Redo),
    ];
    // `WriteCommandLine` is the one action carrying a value an operator wrote,
    // so it is generated rather than picked from the list: a fixed line would
    // leave the only `String` on this enum untested through the codec.
    prop_oneof![
        19 => (0..ACTIONS.len()).prop_map(|index| ACTIONS[index].clone()),
        1 => (".{0,24}", any::<bool>())
            .prop_map(|(line, submit)| Some(A::WriteCommandLine { line, submit })),
    ]
    .boxed()
}

/// A map of between one and `max` arbitrary entries.
///
/// For nested maps whose inner map must not be empty — see the invariant on
/// [`crate::ProgrammerState::values`].
pub fn non_empty_map<K, V>(max: usize) -> impl Strategy<Value = std::collections::BTreeMap<K, V>>
where
    K: Arbitrary + Ord + 'static,
    V: Arbitrary + 'static,
{
    proptest::collection::btree_map(any::<K>(), any::<V>(), 1..max.max(2))
}

/// A vector of at most `max` socket addresses.
///
/// `SocketAddr` has no `Arbitrary` of its own and does not want one: what the
/// output patch holds is a lighting network's node addresses, so the strategy
/// generates IPv4 addresses on plausible ports rather than exploring the shape
/// of the type. See [`crate::OutputKind`].
pub fn sockets(max: usize) -> impl Strategy<Value = Vec<std::net::SocketAddr>> {
    proptest::collection::vec(
        (any::<[u8; 4]>(), any::<u16>()).prop_map(|(octets, port)| {
            std::net::SocketAddr::from((std::net::Ipv4Addr::from(octets), port))
        }),
        0..max,
    )
}

/// One socket address, or none.
///
/// [`sockets`]' reason, for the single address S37's WebSocket listener holds:
/// `SocketAddr` has no `Arbitrary` and should not grow one, because what the
/// domain carries is a lighting network's addresses rather than the shape of the
/// type.
pub fn maybe_socket() -> impl Strategy<Value = Option<std::net::SocketAddr>> {
    proptest::option::of((any::<[u8; 4]>(), any::<u16>()).prop_map(|(octets, port)| {
        std::net::SocketAddr::from((std::net::Ipv4Addr::from(octets), port))
    }))
}

// ---- the whole command vocabulary, in groups ------------------------------

/// Every [`Command`] there is, generated **in groups** so the value tree stays
/// small — S43.
///
/// # Why this is written out rather than derived
///
/// `Command` still derives `Arbitrary`, and that derive is what
/// [`the_grouped_strategy_reaches_every_variant_the_derive_does`] checks this
/// against. What it is no longer used *for* is generating values, and the reason
/// is a measurement rather than a preference.
///
/// `proptest_derive` builds one `TupleUnion` holding a slot per variant, and a
/// slot costs about **576 bytes whatever the variant carries** — S38 found this
/// for a unit enum and S43 measured it again from the other end: boxing a field
/// of a `Command` variant does not move the total by a single byte, because the
/// payload was never what was being paid for. So the tree grew linearly with the
/// number of commands and nothing could be done about it from inside a variant.
/// At fifty-five variants it was 30 992 bytes, which is what a test thread on
/// Windows will just bear; at fifty-seven it was 32 144, and
/// `prism-core`'s `tests/oops.rs` died with `STATUS_STACK_OVERFLOW`. **The
/// protocol had run out of room to grow**, which is not a state a vocabulary
/// should ever be in.
///
/// A `prop_oneof!` of *boxed groups* costs one slot per **group**: six groups of
/// ten measure a few hundred bytes held, and only the group that is chosen is
/// ever built. The measurement is in `crate::wire`, which is where the budget
/// lives.
///
/// # What a session adding a command does
///
/// Add the variant, then add one arm to whichever group has room — and nothing
/// else. `the_grouped_strategy_reaches_every_variant_the_derive_does` fails
/// until you do, by name, so a forgotten arm is a red test rather than a
/// property test that quietly stopped covering something.
/// The type [`command`] returns, so `crate::wire`'s budget can name it.
pub type BoxedCommand = BoxedStrategy<crate::Command>;

pub fn command() -> BoxedCommand {
    prop_oneof![
        command_group_1(),
        command_group_2(),
        command_group_3(),
        command_group_4(),
        command_group_5(),
        command_group_6(),
    ]
    .boxed()
}

/// Group 1 of 6 — see [`command`].
fn command_group_1() -> BoxedStrategy<crate::Command> {
    prop_oneof![
        (small_vec(4), any::<SelectionMode>())
            .prop_map(|(ids, mode)| C::SelectFixtures { ids, mode }),
        (any::<AttributeType>(), any::<i32>(), any::<bool>()).prop_map(
            |(attribute, value, relative)| C::SetAttribute {
                attribute,
                value,
                relative
            }
        ),
        (any::<GroupId>(), any::<SelectionMode>())
            .prop_map(|(group_id, mode)| C::SelectGroup { group_id, mode }),
        any::<PresetId>().prop_map(|preset_id| C::ApplyPreset { preset_id }),
        Just(C::ClearProgrammer),
        (
            any::<Option<SequenceId>>(),
            any::<String>(),
            any::<StoreMode>()
        )
            .prop_map(|(sequence_id, cue_number, mode)| C::StoreCue {
                sequence_id,
                cue_number,
                mode
            }),
        (
            any::<PresetId>(),
            any::<Option<PresetPool>>(),
            any::<String>(),
            any::<Option<RgbColor>>(),
            any::<StoreMode>()
        )
            .prop_map(|(preset_id, pool, name, color, mode)| C::StorePreset {
                preset_id,
                pool,
                name,
                color,
                mode
            }),
        (
            any::<SequenceId>(),
            any::<String>(),
            any::<SequenceStoreMode>()
        )
            .prop_map(|(sequence_id, name, mode)| C::StoreSequence {
                sequence_id,
                name,
                mode
            }),
        (any::<GroupId>(), any::<String>(), any::<OverwriteMode>()).prop_map(
            |(group_id, name, mode)| C::StoreGroup {
                group_id,
                name,
                mode
            }
        ),
        (any::<Option<SequenceId>>(), any::<String>()).prop_map(|(sequence_id, cue_number)| {
            C::EditCue {
                sequence_id,
                cue_number,
            }
        }),
    ]
    .boxed()
}

/// Group 2 of 6 — see [`command`].
fn command_group_2() -> BoxedStrategy<crate::Command> {
    prop_oneof![
        Just(C::Update),
        (
            any::<Option<SequenceId>>(),
            any::<String>(),
            any::<CueProperty>()
        )
            .prop_map(|(sequence_id, cue_number, property)| C::SetCueProperty {
                sequence_id,
                cue_number,
                property
            }),
        (
            any::<Option<SequenceId>>(),
            any::<String>(),
            any::<crate::CueTrackingMode>()
        )
            .prop_map(|(sequence_id, cue_number, tracking)| C::SetCueTracking {
                sequence_id,
                cue_number,
                tracking
            }),
        any::<ObjectRef>().prop_map(|target| C::Delete { target }),
        (
            any::<ObjectRef>(),
            any::<ObjectRef>(),
            any::<OverwriteMode>()
        )
            .prop_map(|(from, to, mode)| C::Copy { from, to, mode }),
        (
            any::<ObjectRef>(),
            any::<ObjectRef>(),
            any::<OverwriteMode>()
        )
            .prop_map(|(from, to, mode)| C::Move { from, to, mode }),
        (any::<ObjectRef>(), any::<String>()).prop_map(|(target, name)| C::Label { target, name }),
        (any::<ObjectRef>(), any::<Option<RgbColor>>())
            .prop_map(|(target, color)| C::Color { target, color }),
        (any::<PlaybackTarget>(), any::<String>())
            .prop_map(|(target, cue_number)| C::Goto { target, cue_number }),
        any::<PlaybackTarget>().prop_map(|target| C::ExecutorOn { target }),
        (any::<ExecutorId>(), any::<Option<SequenceId>>()).prop_map(
            |(executor_id, sequence_id)| C::AssignExecutor {
                executor_id,
                sequence_id
            }
        ),
        (any::<ExecutorId>(), any::<crate::ExecutorChange>()).prop_map(|(executor_id, change)| {
            C::ConfigureExecutor {
                executor_id,
                change,
            }
        }),
    ]
    .boxed()
}

/// Group 3 of 6 — see [`command`].
fn command_group_3() -> BoxedStrategy<crate::Command> {
    prop_oneof![
        (any::<PlaybackTarget>(), any::<GoDirection>())
            .prop_map(|(target, direction)| C::ExecutorGo { target, direction }),
        any::<PlaybackTarget>().prop_map(|target| C::ExecutorOff { target }),
        (
            any::<ExecutorId>(),
            any::<ExecutorButtonRef>(),
            any::<bool>()
        )
            .prop_map(|(executor_id, button, pressed)| C::ExecutorButton {
                executor_id,
                button,
                pressed
            }),
        (any::<ExecutorId>(), any::<u16>())
            .prop_map(|(executor_id, level)| C::SetExecutorMaster { executor_id, level }),
        (
            any::<FixtureId>(),
            any::<String>(),
            any::<String>(),
            any::<UniverseId>(),
            any::<u16>()
        )
            .prop_map(|(id, name, type_id, universe, address)| C::PatchFixture {
                software_dimmer: true,
                id,
                name,
                type_id,
                universe,
                address
            }),
        any::<FixtureId>().prop_map(|id| C::UnpatchFixture { id }),
        (any::<FixtureId>(), any::<FixtureId>()).prop_map(|(id, to)| C::RenumberFixture { id, to }),
        any::<String>().prop_map(|type_id| C::EmbedFixtureType { type_id }),
        Just(C::Oops),
        Just(C::Redo),
    ]
    .boxed()
}

/// Group 4 of 6 — see [`command`].
fn command_group_4() -> BoxedStrategy<crate::Command> {
    prop_oneof![
        Just(C::SaveShow),
        any::<String>().prop_map(|path| C::SaveShowAs { path }),
        any::<String>().prop_map(|path| C::OpenShow { path }),
        any::<String>().prop_map(|path| C::NewShow { path }),
        any::<String>().prop_map(|path| C::ExportShow { path }),
        any::<String>().prop_map(|path| C::ImportShow { path }),
        any::<ViewId>().prop_map(|view_id| C::SelectView { view_id }),
        (any::<ViewId>(), any::<String>())
            .prop_map(|(view_id, name)| C::StoreView { view_id, name }),
        (any::<ViewId>(), any::<String>()).prop_map(|(view_id, name)| C::NewView { view_id, name }),
        any::<bool>().prop_map(|open| C::SetWindowPicker { open }),
    ]
    .boxed()
}

/// Group 5 of 6 — see [`command`].
fn command_group_5() -> BoxedStrategy<crate::Command> {
    prop_oneof![
        (any::<WindowType>(), proptest::option::of(small_map(2)))
            .prop_map(|(window, params)| C::OpenWindow { window, params }),
        any::<WindowInstanceId>().prop_map(|instance_id| C::CloseWindow { instance_id }),
        any::<WindowInstanceId>().prop_map(|instance_id| C::FocusWindow { instance_id }),
        (
            any::<WindowInstanceId>(),
            finite_f64(),
            finite_f64(),
            finite_f64(),
            finite_f64()
        )
            .prop_map(|(instance_id, x, y, w, h)| C::PlaceWindow {
                instance_id,
                x,
                y,
                w,
                h
            }),
        any::<u32>().prop_map(|page| C::SetExecutorPage { page }),
        any::<ExecutorId>().prop_map(|executor_id| C::SelectExecutor { executor_id }),
        any::<SequenceId>().prop_map(|sequence_id| C::SelectSequence { sequence_id }),
        any::<FeatureGroup>().prop_map(|group| C::SetEncoderBank { group }),
        any::<u32>().prop_map(|page| C::SetProgrammerPage { page }),
        any::<ParamDirection>().prop_map(|direction| C::SelectProgrammerParam { direction }),
    ]
    .boxed()
}

/// Group 6 of 6 — see [`command`].
fn command_group_6() -> BoxedStrategy<crate::Command> {
    prop_oneof![
        (
            any::<String>(),
            any::<bool>(),
            any::<Option<crate::CommandLineMode>>()
        )
            .prop_map(|(text, run, mode)| C::CommandLineInput { text, run, mode }),
        any::<OutputInstance>().prop_map(|output| C::AddOutput { output }),
        (any::<OutputId>(), any::<OutputChange>())
            .prop_map(|(id, change)| C::ConfigureOutput { id, change }),
        any::<OutputId>().prop_map(|id| C::RemoveOutput { id }),
        (any::<OutputId>(), any::<bool>())
            .prop_map(|(id, enabled)| C::SetOutputEnabled { id, enabled }),
        any::<Option<String>>().prop_map(|port| C::SetSurfacePort { port }),
        any::<MachineChange>().prop_map(|change| C::ConfigureMachine { change }),
    ]
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::command;
    use proptest::prelude::*;
    use proptest::strategy::ValueTree;
    use proptest::test_runner::TestRunner;
    use std::collections::BTreeSet;

    /// Which variant a command is, by the tag `Command` puts on the wire.
    fn tag(command: &crate::Command) -> String {
        let value = serde_json::to_value(command).expect("a command serialises");
        value["t"].as_str().expect("a tagged command").to_owned()
    }

    /// The variants a strategy reaches, over enough samples to reach them all.
    fn reached<S: Strategy<Value = crate::Command>>(strategy: &S) -> BTreeSet<String> {
        let mut runner = TestRunner::deterministic();
        let mut seen = BTreeSet::new();
        for _ in 0..3_000 {
            let tree = strategy.new_tree(&mut runner).expect("a value");
            seen.insert(tag(&tree.current()));
        }
        seen
    }

    /// **The guard that replaces the compiler's exhaustiveness check.**
    ///
    /// [`command`] is written out by hand, so a variant added to `Command`
    /// without an arm would be a variant no property test ever generated —
    /// silently, which is the one failure mode this arrangement has. `Command`'s
    /// own derive is exhaustive by construction, so comparing what the two reach
    /// turns that silence into a named failure.
    ///
    /// The derived strategy is only *constructed* here, one value at a time,
    /// which is what a proptest run does anyway; it is never the thing a
    /// property is run over. See `crate::wire`'s budget for why.
    #[test]
    fn the_grouped_strategy_reaches_every_variant_the_derive_does() {
        let grouped = reached(&command());
        let derived = reached(&any::<crate::Command>());
        let missing: Vec<&String> = derived.difference(&grouped).collect();
        assert!(
            missing.is_empty(),
            "crate::arb::command has no arm for {missing:?} — add one to a group"
        );
        let extra: Vec<&String> = grouped.difference(&derived).collect();
        assert!(
            extra.is_empty(),
            "crate::arb::command generates {extra:?}, which Command does not have"
        );
    }
}
