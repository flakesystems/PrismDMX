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
    const ACTIONS: [Option<A>; 18] = [
        None,
        Some(A::ExecutorMaster {
            target: ExecutorTarget::Strip,
        }),
        Some(A::ExecutorGo {
            target: ExecutorTarget::Selected,
            direction: crate::GoDirection::Next,
        }),
        Some(A::ExecutorOff {
            target: ExecutorTarget::Strip,
        }),
        Some(A::ExecutorButton {
            target: ExecutorTarget::Strip,
            button: crate::ExecutorButtonRef::Slot { index: 0 },
        }),
        Some(A::SelectExecutor {
            target: ExecutorTarget::Strip,
        }),
        Some(A::ClearProgrammer),
        Some(A::ExecutorPage { delta: -1 }),
        Some(A::SelectView {
            view: crate::ViewId::new(1),
        }),
        Some(A::StepView {
            direction: Step::Next,
        }),
        Some(A::ProgrammerPage { delta: 1 }),
        Some(A::SelectProgrammerParam {
            direction: crate::ParamDirection::Prev,
        }),
        Some(A::AdjustParameter),
        Some(A::SetEncoderBank {
            group: crate::FeatureGroup::Color,
        }),
        Some(A::OpenWindow {
            window: crate::WindowType::Patch,
        }),
        Some(A::SaveShow),
        Some(A::Oops),
        Some(A::Redo),
    ];
    (0..ACTIONS.len()).prop_map(|index| ACTIONS[index]).boxed()
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
