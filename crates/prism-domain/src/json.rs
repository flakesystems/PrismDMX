//! A schema-free value and RFC 6902 patch operations.
//!
//! `docs/IPC_PROTOCOL.md` §6 sends `ShowPatch` and `SessionPatch` as lists of
//! JSON Patch operations, and `ARCHITECTURE_SPEC.md` §6 gives `WindowInstance`
//! a `params: Record<string, unknown>` bag. Both need a value type that is not
//! statically known.
//!
//! `serde_json::Value` would do the job in Rust, but `ts-rs` renders it as
//! `any`, which `CLAUDE.md` forbids. [`JsonValue`] is therefore defined here and
//! renders as a closed union.
//!
//! # Nesting depth is a transport concern
//!
//! [`JsonValue`] is recursive, so a corrupt or hostile payload can nest
//! arbitrarily deep and exhaust the stack while being decoded — and a stack
//! overflow aborts the process, which would take DMX output down with it.
//!
//! That limit cannot be enforced here. `serde` buffers the content of an
//! internally tagged enum before any of this crate's code runs, so by the time
//! a `JsonValue` inside a `Delta` is constructed, the recursion has already
//! happened. `serde_json` applies its own limit (128 levels); MessagePack has
//! none by default. **`prism-ipc` (S16) must therefore enforce a nesting depth
//! limit on decode**, next to the maximum frame size it already rejects on.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A value whose shape is not known at compile time.
///
/// Untagged, so it is indistinguishable from plain JSON on the wire. Variant
/// order matters for deserialisation: `7` must come back as [`JsonValue::Int`]
/// and `7.5` as [`JsonValue::Float`], so `Int` is tried first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(untagged)]
pub enum JsonValue {
    /// JSON `null`.
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// A whole number.
    Int(i64),
    /// A finite floating-point number. JSON cannot encode NaN or infinity, so
    /// both are rejected in either direction — see [`crate::finite`].
    Float(
        #[serde(with = "crate::finite")]
        #[ts(as = "f64")]
        f64,
    ),
    /// A string.
    String(String),
    /// An ordered list.
    Array(Vec<JsonValue>),
    /// A key-ordered object.
    Object(BTreeMap<String, JsonValue>),
}

impl JsonValue {
    /// Strategy for property tests: bounded depth and width, finite floats only.
    #[cfg(any(test, feature = "proptest"))]
    fn strategy() -> impl proptest::strategy::Strategy<Value = Self> {
        use proptest::prelude::*;

        let leaf = prop_oneof![
            Just(Self::Null),
            any::<bool>().prop_map(Self::Bool),
            any::<i32>().prop_map(|value| Self::Int(i64::from(value))),
            crate::arb::finite_f64().prop_map(Self::Float),
            ".{0,8}".prop_map(Self::String),
        ];
        leaf.prop_recursive(3, 12, 3, |inner| {
            prop_oneof![
                proptest::collection::vec(inner.clone(), 0..3).prop_map(Self::Array),
                proptest::collection::btree_map(".{0,8}", inner, 0..3).prop_map(Self::Object),
            ]
        })
    }
}

#[cfg(any(test, feature = "proptest"))]
impl proptest::arbitrary::Arbitrary for JsonValue {
    type Parameters = ();
    type Strategy = proptest::strategy::BoxedStrategy<Self>;

    fn arbitrary_with((): Self::Parameters) -> Self::Strategy {
        use proptest::strategy::Strategy;
        Self::strategy().boxed()
    }
}

/// One RFC 6902 JSON Patch operation.
///
/// `prism-core` generates these when a command changes the show or the session;
/// clients apply them to their mirror verbatim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum JsonPatchOp {
    /// Insert `value` at `path`.
    Add {
        /// JSON Pointer to the target location.
        path: String,
        /// The value to insert.
        value: JsonValue,
    },
    /// Delete the value at `path`.
    Remove {
        /// JSON Pointer to the target location.
        path: String,
    },
    /// Overwrite the value at `path`.
    Replace {
        /// JSON Pointer to the target location.
        path: String,
        /// The new value.
        value: JsonValue,
    },
    /// Move the value at `from` to `path`.
    Move {
        /// JSON Pointer to the source location.
        from: String,
        /// JSON Pointer to the target location.
        path: String,
    },
    /// Copy the value at `from` to `path`.
    Copy {
        /// JSON Pointer to the source location.
        from: String,
        /// JSON Pointer to the target location.
        path: String,
    },
    /// Assert that `path` holds `value`.
    Test {
        /// JSON Pointer to the target location.
        path: String,
        /// The expected value.
        value: JsonValue,
    },
}

impl JsonPatchOp {
    /// The JSON Pointer this operation writes to.
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            Self::Add { path, .. }
            | Self::Remove { path }
            | Self::Replace { path, .. }
            | Self::Move { path, .. }
            | Self::Copy { path, .. }
            | Self::Test { path, .. } => path,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{JsonPatchOp, JsonValue};
    use std::collections::BTreeMap;
    use ts_rs::{Config, TS};

    fn json(value: &JsonValue) -> String {
        serde_json::to_string(value).unwrap()
    }

    #[test]
    fn values_are_untagged_on_the_wire() {
        assert_eq!(json(&JsonValue::Null), "null");
        assert_eq!(json(&JsonValue::Bool(true)), "true");
        assert_eq!(json(&JsonValue::Int(-3)), "-3");
        assert_eq!(json(&JsonValue::Float(1.5)), "1.5");
        assert_eq!(json(&JsonValue::String("a".into())), "\"a\"");
        assert_eq!(
            json(&JsonValue::Array(vec![JsonValue::Int(1), JsonValue::Null])),
            "[1,null]"
        );
        let object = BTreeMap::from([("k".to_owned(), JsonValue::Bool(false))]);
        assert_eq!(json(&JsonValue::Object(object)), r#"{"k":false}"#);
    }

    #[test]
    fn integers_stay_integers_through_a_round_trip() {
        let value: JsonValue = serde_json::from_str("7").unwrap();
        assert_eq!(value, JsonValue::Int(7));
        let value: JsonValue = serde_json::from_str("7.5").unwrap();
        assert_eq!(value, JsonValue::Float(7.5));
    }

    #[test]
    fn patch_operations_match_rfc_6902() {
        let add = JsonPatchOp::Add {
            path: "/fixtures/1".to_owned(),
            value: JsonValue::Int(1),
        };
        assert_eq!(
            serde_json::to_string(&add).unwrap(),
            r#"{"op":"add","path":"/fixtures/1","value":1}"#
        );

        let remove = JsonPatchOp::Remove {
            path: "/fixtures/1".to_owned(),
        };
        assert_eq!(
            serde_json::to_string(&remove).unwrap(),
            r#"{"op":"remove","path":"/fixtures/1"}"#
        );

        let mv = JsonPatchOp::Move {
            from: "/a".to_owned(),
            path: "/b".to_owned(),
        };
        assert_eq!(
            serde_json::to_string(&mv).unwrap(),
            r#"{"op":"move","from":"/a","path":"/b"}"#
        );
    }

    #[test]
    fn every_operation_names_the_path_it_writes_to() {
        let ops = [
            JsonPatchOp::Add {
                path: "/a".to_owned(),
                value: JsonValue::Null,
            },
            JsonPatchOp::Remove {
                path: "/a".to_owned(),
            },
            JsonPatchOp::Replace {
                path: "/a".to_owned(),
                value: JsonValue::Null,
            },
            JsonPatchOp::Move {
                from: "/b".to_owned(),
                path: "/a".to_owned(),
            },
            JsonPatchOp::Copy {
                from: "/b".to_owned(),
                path: "/a".to_owned(),
            },
            JsonPatchOp::Test {
                path: "/a".to_owned(),
                value: JsonValue::Null,
            },
        ];
        for op in &ops {
            assert_eq!(op.path(), "/a");
        }
    }

    #[test]
    fn typescript_bindings_contain_no_any() {
        let cfg = Config::new();
        assert!(!JsonValue::decl(&cfg).contains("any"));
        assert!(!JsonPatchOp::decl(&cfg).contains("any"));
    }
}
